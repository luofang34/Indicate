// Exercise the generated gallery in an isolated browser profile.
import { spawn } from 'node:child_process';
import { mkdtemp, rm, writeFile, mkdir } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { pathToFileURL } from 'node:url';
import assert from 'node:assert/strict';

const chromeBinary = process.env.CHROME_BINARY || '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome';
const pagePath = resolve(process.argv[2] || 'target/hwd-flight-review.html');
const output = resolve(process.argv[3] || 'target/hwd-browser-review');
await mkdir(output, { recursive: true });
const profile = await mkdtemp(join(tmpdir(), 'indicate-hwd-review-'));
const chrome = spawn(chromeBinary, [
    '--headless=new', '--disable-gpu', '--no-first-run', '--no-default-browser-check',
    '--disable-background-networking', '--remote-debugging-port=0', `--user-data-dir=${profile}`, 'about:blank',
], { stdio: ['ignore', 'ignore', 'pipe'] });
let socket, serial = 0, session, fatal = [];
const pending = new Map(), events = new Map();
const deadline = setTimeout(() => chrome.kill('SIGKILL'), 45000);
try {
    const endpoint = await new Promise((resolveEndpoint, reject) => {
        let stderr = '';
        chrome.once('error', reject);
        chrome.once('exit', code => reject(new Error(`Chrome exited before inspection: ${code}`)));
        chrome.stderr.on('data', data => {
            stderr += data;
            const match = stderr.match(/DevTools listening on (ws:\/\/[^\s]+)/);
            if (match) resolveEndpoint(match[1]);
        });
    });
    socket = new WebSocket(endpoint);
    await new Promise((resolveOpen, reject) => {
        socket.addEventListener('open', resolveOpen, { once: true });
        socket.addEventListener('error', reject, { once: true });
    });
    socket.addEventListener('message', event => {
        const message = JSON.parse(event.data);
        if (message.id) {
            const request = pending.get(message.id);
            pending.delete(message.id);
            if (message.error) request.reject(new Error(JSON.stringify(message.error)));
            else request.resolve(message.result);
        } else {
            if (message.method === 'Runtime.exceptionThrown') fatal.push(message.params);
            const callback = events.get(message.method);
            if (callback) { events.delete(message.method); callback(message.params); }
        }
    });
    const target = await call('Target.createTarget', { url: 'about:blank' }, false);
    session = (await call('Target.attachToTarget', { targetId: target.targetId, flatten: true }, false)).sessionId;
    await call('Page.enable');
    await call('Runtime.enable');
    await call('Emulation.setDeviceMetricsOverride', { width: 1700, height: 1400, deviceScaleFactor: 1, mobile: false });
    const loaded = new Promise(resolveLoad => events.set('Page.loadEventFired', resolveLoad));
    await call('Page.navigate', { url: pathToFileURL(pagePath).href + '#42' });
    await loaded;
    assert.equal(await evaluate('cases.length'), 55);
    assert.equal(await evaluate('profiles.length'), 6);
    assert.equal(await evaluate('shapes.every(shape => shape.reference === 2)'), true);
    const projectionError = await evaluate(`(() => {
        let error = 0;
        for (const yaw of [0, 90, 180, 270, 360]) for (const pitch of [-85, 0, 85]) for (const roll of [-60, 0, 60]) {
            const head = basis(yaw, pitch, roll);
            for (const point of [[0,0], [1200,0], [0,600], [1200,600], [600,300]]) {
                const p = project(plane(point, head), head);
                error = Math.max(error, Math.abs(p[0] - point[0]), Math.abs(p[1] - point[1] - 60));
            }
        }
        return error;
    })()`);
    assert.ok(projectionError < 1e-8, 'text reservations must survive head yaw, pitch, and roll');
    const started = await evaluate('eventStart');
    await evaluate("document.querySelector('#profile').value='2'; document.querySelector('#profile').dispatchEvent(new Event('input'));");
    assert.equal(await evaluate('eventStart'), started, 'profile changes cannot restart event attention');
    await evaluate("document.querySelector('#zones').checked=true; draw();");
    assert.equal(await evaluate('cases[current].profiles[2].mission'), true);
    await screenshot('mission');
    await evaluate("document.querySelector('#profile').value='3'; draw();");
    assert.equal(await evaluate('cases[current].profiles[3].blank'), true);
    assert.match(await evaluate("document.querySelector('#mode').textContent"), /BLANKED/);
    await screenshot('hud-blank');
    await evaluate("document.querySelector('#profile').value='5'; draw();");
    assert.equal(await evaluate('cases[current].profiles[5].blank'), false);
    await evaluate("document.querySelector('#profile').value='3'; select(50);");
    assert.equal(await evaluate('cases[current].profiles[3].blank'), false);
    assert.match(await evaluate("document.querySelector('#mode').textContent"), /RECOVERY/);
    await screenshot('recovery');
    for (let index = 51; index < 55; index++) {
        await evaluate(`select(${index});`);
        assert.match(await evaluate("document.querySelector('#mode').textContent"), /RECOVERY/);
        await screenshot(`forward-recovery-${index + 1}`);
    }
    await evaluate("document.querySelector('#profile').value='0'; document.querySelector('#zones').checked=false; select(42);");
    const attentionPixels = 'Array.from(ctx.getImageData(170,62,170,24).data).reduce((sum, value, index) => sum + value * (index + 1), 0)';
    const highlighted = await evaluate(attentionPixels);
    await evaluate(`new Promise(resolveTimer => setTimeout(resolveTimer, cases[current].attentionMs + 150))`, true);
    assert.equal(await evaluate('performance.now() - eventStart >= cases[current].attentionMs'), true);
    assert.notEqual(await evaluate(attentionPixels), highlighted, 'expired attention must clear without another user action');
    await screenshot('flight-settled');
    for (const [index, name] of [[28, 'navigation-clearance'], [46, 'envelope-clearance']]) {
        await evaluate(`select(${index});`);
        await screenshot(name);
    }
    assert.deepEqual(fatal, [], 'browser JavaScript exceptions');
    process.stdout.write(JSON.stringify({ cases: 55, profiles: 6, result: 'passed', screenshots: output }) + '\n');
    await call('Target.closeTarget', { targetId: target.targetId }, false);
} finally {
    clearTimeout(deadline);
    if (socket) socket.close();
    chrome.kill('SIGKILL');
    await new Promise(resolveExit => chrome.exitCode !== null ? resolveExit() : chrome.once('exit', resolveExit));
    await rm(profile, { recursive: true, force: true, maxRetries: 3 });
}

function call(method, params = {}, attached = true) {
    const id = ++serial;
    return new Promise((resolveRequest, reject) => {
        pending.set(id, { resolve: resolveRequest, reject });
        socket.send(JSON.stringify({ id, method, params, ...(attached ? { sessionId: session } : {}) }));
    });
}
async function evaluate(expression, awaitPromise = false) {
    const response = await call('Runtime.evaluate', { expression, awaitPromise, returnByValue: true });
    if (response.exceptionDetails) throw new Error(JSON.stringify(response.exceptionDetails));
    return response.result.value;
}
async function screenshot(name) {
    const { data } = await call('Page.captureScreenshot', { format: 'png' });
    await writeFile(join(output, name + '.png'), Buffer.from(data, 'base64'));
}
