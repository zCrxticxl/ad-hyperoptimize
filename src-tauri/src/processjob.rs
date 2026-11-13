//! Contain checked commands before their first instruction can spawn children.
use std::ffi::c_void;
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
use std::process::Child;

#[repr(C)]
#[derive(Default)]
struct BasicLimits {
    process_time: i64,
    job_time: i64,
    flags: u32,
    min_working_set: usize,
    max_working_set: usize,
    active_processes: u32,
    affinity: usize,
    priority: u32,
    scheduling: u32,
}
#[repr(C)]
#[derive(Default)]
struct ExtendedLimits {
    basic: BasicLimits,
    io: [u64; 6],
    process_memory: usize,
    job_memory: usize,
    peak_process_memory: usize,
    peak_job_memory: usize,
}
#[repr(C)]
#[derive(Default)]
struct ThreadEntry {
    size: u32,
    usage: u32,
    id: u32,
    process: u32,
    base_priority: i32,
    delta_priority: i32,
    flags: u32,
}

#[link(name = "kernel32")]
extern "system" {
    fn CreateJobObjectW(attributes: *mut c_void, name: *const u16) -> *mut c_void;
    fn SetInformationJobObject(job: *mut c_void, class: i32, info: *const c_void, len: u32) -> i32;
    fn AssignProcessToJobObject(job: *mut c_void, process: *mut c_void) -> i32;
    fn CreateToolhelp32Snapshot(flags: u32, pid: u32) -> *mut c_void;
    fn Thread32First(snapshot: *mut c_void, entry: *mut ThreadEntry) -> i32;
    fn Thread32Next(snapshot: *mut c_void, entry: *mut ThreadEntry) -> i32;
    fn OpenThread(access: u32, inherit: i32, id: u32) -> *mut c_void;
    fn ResumeThread(thread: *mut c_void) -> u32;
    fn TerminateJobObject(job: *mut c_void, code: u32) -> i32;
}

fn owned(raw: *mut c_void) -> Result<OwnedHandle, String> {
    if raw.is_null() || raw == -1isize as *mut c_void {
        return Err(std::io::Error::last_os_error().to_string());
    }
    // SAFETY: callers pass newly created Win32 handles and transfer ownership.
    Ok(unsafe { OwnedHandle::from_raw_handle(raw) })
}

pub(crate) struct Job(OwnedHandle);

impl Job {
    pub(crate) fn terminate(&self) -> std::io::Result<()> {
        // SAFETY: self owns a valid job handle for this command tree only.
        if unsafe { TerminateJobObject(self.0.as_raw_handle(), 1) } == 0 {
            return Err(std::io::Error::last_os_error());
        }
        Ok(())
    }
}

pub(crate) fn contain_and_resume(child: &mut Child) -> Result<Job, String> {
    let result = (|| {
        // SAFETY: unnamed job with default security; all buffers below have Win32 layouts.
        let job = owned(unsafe { CreateJobObjectW(std::ptr::null_mut(), std::ptr::null()) })?;
        let mut limits = ExtendedLimits::default();
        limits.basic.flags = 0x2000; // JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE
        if unsafe {
            SetInformationJobObject(
                job.as_raw_handle(),
                9,
                &limits as *const _ as *const c_void,
                std::mem::size_of::<ExtendedLimits>() as u32,
            )
        } == 0
        {
            return Err(std::io::Error::last_os_error().to_string());
        }
        if unsafe { AssignProcessToJobObject(job.as_raw_handle(), child.as_raw_handle()) } == 0 {
            return Err(std::io::Error::last_os_error().to_string());
        }
        // CREATE_SUSPENDED leaves exactly the primary thread to resume.
        let snapshot = owned(unsafe { CreateToolhelp32Snapshot(4, 0) })?;
        let mut entry = ThreadEntry {
            size: std::mem::size_of::<ThreadEntry>() as u32,
            ..Default::default()
        };
        let mut found = unsafe { Thread32First(snapshot.as_raw_handle(), &mut entry) };
        while found != 0 {
            if entry.process == child.id() {
                let thread = owned(unsafe { OpenThread(2, 0, entry.id) })?;
                if unsafe { ResumeThread(thread.as_raw_handle()) } == u32::MAX {
                    return Err(std::io::Error::last_os_error().to_string());
                }
                return Ok(Job(job));
            }
            found = unsafe { Thread32Next(snapshot.as_raw_handle(), &mut entry) };
        }
        Err("Cannot find suspended command's primary thread".into())
    })();
    if result.is_err() {
        let _ = child.kill();
        let _ = child.wait();
    }
    result
}
