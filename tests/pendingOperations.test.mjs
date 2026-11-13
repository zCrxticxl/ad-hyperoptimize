import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import ts from 'typescript';

const source = readFileSync(new URL('../src/lib/pendingOperations.ts', import.meta.url), 'utf8');
const js = ts.transpileModule(source, { compilerOptions: { module: ts.ModuleKind.ESNext } }).outputText;
const { pendingOperations, beginOperation, trackOperation } = await import(`data:text/javascript;base64,${Buffer.from(js).toString('base64')}`);

test('pending work blocks navigation until every operation settles', async () => {
  const snapshots = [];
  const unsubscribe = pendingOperations.subscribe(() => snapshots.push(pendingOperations.getSnapshot()));
  let resolve;
  const promise = trackOperation(() => new Promise(r => { resolve = r; }));
  const end = beginOperation();
  assert.equal(pendingOperations.getSnapshot(), 2);
  end(); end();
  assert.equal(pendingOperations.getSnapshot(), 1);
  resolve('result');
  assert.equal(await promise, 'result');
  assert.equal(pendingOperations.getSnapshot(), 0);
  assert.deepEqual(snapshots, [1, 2, 1, 0]);
  unsubscribe();
});

test('rejections and synchronous throws release the guard without hiding errors', async () => {
  await assert.rejects(trackOperation(() => Promise.reject(new Error('reject'))), /reject/);
  await assert.rejects(trackOperation(() => { throw new Error('throw'); }), /throw/);
  assert.equal(pendingOperations.getSnapshot(), 0);
});
