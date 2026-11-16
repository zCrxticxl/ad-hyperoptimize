$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
Add-Type -AssemblyName System.Net.Http
Add-Type -ReferencedAssemblies System.Net.Http -TypeDefinition @'
using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.Net;
using System.Net.Http;
using System.Net.NetworkInformation;
using System.Threading;
using System.Threading.Tasks;
public class NetProbeSample {
    public string target;
    public string status;
    public double? ms;
    public bool loadActive;
}
public class NetProbePhase {
    public List<NetProbeSample> samples = new List<NetProbeSample>();
    public double seconds;
    public double rxMbps;
    public double txMbps;
}
public class NetProbeResult {
    public NetProbePhase idle;
    public NetProbePhase loaded;
    public long downloadBytes;
    public double downloadSeconds;
    public string loadError;
    public bool byteLimitReached;
}
public static class AdNetworkProbe {
    private class Load {
        public long bytes;
        public double seconds;
        public string error;
    }
    private static async Task Download(Load load, CancellationToken token) {
        var watch = Stopwatch.StartNew();
        try {
            using (var handler = new HttpClientHandler { AllowAutoRedirect = false, UseCookies = false })
            using (var client = new HttpClient(handler)) {
                client.Timeout = TimeSpan.FromSeconds(12);
                using (var response = await client.GetAsync("https://speed.cloudflare.com/__down?bytes=67108864", HttpCompletionOption.ResponseHeadersRead, token)) {
                    if (response.StatusCode != HttpStatusCode.OK) throw new Exception("HTTP " + (int)response.StatusCode);
                    using (var stream = await response.Content.ReadAsStreamAsync()) {
                        byte[] buffer = new byte[65536];
                        while (load.bytes < 67108864) {
                            int read = await stream.ReadAsync(buffer, 0, (int)Math.Min(buffer.Length, 67108864 - load.bytes), token);
                            if (read == 0) break;
                            Interlocked.Add(ref load.bytes, read);
                        }
                    }
                }
            }
        } catch (OperationCanceledException) { }
        catch (Exception e) { load.error = e.Message; }
        finally { load.seconds = watch.Elapsed.TotalSeconds; }
    }
    private static async Task<NetProbeSample> PingOne(string target) {
        using (var ping = new Ping()) {
            try {
                var reply = await ping.SendPingAsync(target, 700);
                return new NetProbeSample { target = target, status = reply.Status.ToString(), ms = reply.Status == IPStatus.Success ? (double?)reply.RoundtripTime : null };
            } catch (Exception e) { return new NetProbeSample { target = target, status = "LocalError: " + e.Message }; }
        }
    }
    private static NetProbePhase Measure(NetworkInterface adapter, string[] targets, Task load) {
        var phase = new NetProbePhase();
        var before = adapter.GetIPv4Statistics();
        var watch = Stopwatch.StartNew();
        for (int i = 0; i < 16; i++) {
            if (load != null && load.IsCompleted) break;
            var round = Stopwatch.StartNew();
            var tasks = new List<Task<NetProbeSample>>();
            foreach (var target in targets) tasks.Add(PingOne(target));
            Task.WaitAll(tasks.ToArray());
            foreach (var task in tasks) {
                task.Result.loadActive = load != null && !load.IsCompleted;
                phase.samples.Add(task.Result);
            }
            Thread.Sleep(Math.Max(0, 500 - (int)round.ElapsedMilliseconds));
        }
        var after = adapter.GetIPv4Statistics();
        phase.seconds = watch.Elapsed.TotalSeconds;
        phase.rxMbps = Math.Max(0, after.BytesReceived - before.BytesReceived) * 8.0 / Math.Max(0.001, phase.seconds) / 1000000;
        phase.txMbps = Math.Max(0, after.BytesSent - before.BytesSent) * 8.0 / Math.Max(0.001, phase.seconds) / 1000000;
        return phase;
    }
    public static NetProbeResult Run(string adapterId, string gateway, bool loadEnabled) {
        var adapter = Array.Find(NetworkInterface.GetAllNetworkInterfaces(), a => String.Equals(a.Id.Trim('{','}'), adapterId.Trim('{','}'), StringComparison.OrdinalIgnoreCase));
        if (adapter == null) throw new Exception("Selected network adapter disappeared");
        var targets = new List<string>();
        IPAddress ip;
        if (IPAddress.TryParse(gateway, out ip) && !ip.Equals(IPAddress.Any)) targets.Add(ip.ToString());
        targets.Add("1.1.1.1"); targets.Add("8.8.8.8");
        var result = new NetProbeResult();
        result.idle = Measure(adapter, targets.ToArray(), null);
        if (loadEnabled) {
            var load = new Load();
            using (var cancel = new CancellationTokenSource(12000)) {
                var task = Download(load, cancel.Token);
                var warm = Stopwatch.StartNew();
                while (!task.IsCompleted && Interlocked.Read(ref load.bytes) == 0 && warm.ElapsedMilliseconds < 4000) Thread.Sleep(25);
                result.loaded = Interlocked.Read(ref load.bytes) > 0 ? Measure(adapter, targets.ToArray(), task) : new NetProbePhase();
                cancel.Cancel();
                task.GetAwaiter().GetResult();
            }
            result.downloadBytes = load.bytes;
            result.downloadSeconds = load.seconds;
            result.loadError = load.error;
            result.byteLimitReached = load.bytes >= 67108864;
        }
        return result;
    }
}
'@
$route = Find-NetRoute -RemoteIPAddress '1.1.1.1' -ErrorAction Stop | Where-Object { $null -ne $_.NextHop } | Select-Object -First 1
if (!$route) { throw 'No IPv4 route to the test target' }
$adapter = Get-NetAdapter -InterfaceIndex $route.InterfaceIndex -ErrorAction Stop
$wifi = $null
$wifiError = $null
if ($adapter.NdisPhysicalMedium -eq 9 -or $adapter.NdisPhysicalMedium -eq 1 -or $adapter.InterfaceDescription -match 'Wireless|Wi-Fi|WiFi|802.11') {
    $wifi = (& "$env:SystemRoot\System32\netsh.exe" wlan show interfaces 2>&1 | Out-String).Trim()
    if ($LASTEXITCODE -ne 0) { $wifiError = $wifi; $wifi = $null }
}
$result = [AdNetworkProbe]::Run([string]$adapter.InterfaceGuid, [string]$route.NextHop, __LOAD__)
@{ adapter = $adapter.Name; description = $adapter.InterfaceDescription; linkSpeed = [string]$adapter.LinkSpeed; gateway = [string]$route.NextHop; wifiDetails = $wifi; wifiError = $wifiError; measurements = $result } | ConvertTo-Json -Depth 8 -Compress
