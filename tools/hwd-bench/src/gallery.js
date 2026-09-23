const cases = [__CASES__], shapes = [__SHAPES__], profiles = [__PROFILES__], zones = [__ZONES__];
const canvas = document.querySelector('#view'), ctx = canvas.getContext('2d'), rad = Math.PI / 180;
const inputs = ['yaw', 'pitch', 'roll'].map(id => document.getElementById(id));
let current = 0, compact = false, running = false, eventStart = performance.now(), last = 0, eventTimer = null;
const dot = (a, b) => a.reduce((s, v, i) => s + v * b[i], 0);
const add = (a, b) => a.map((v, i) => v + b[i]), mul = (a, k) => a.map(v => v * k);
const cross = (a, b) => [a[1]*b[2]-a[2]*b[1], a[2]*b[0]-a[0]*b[2], a[0]*b[1]-a[1]*b[0]];
function basis(yaw, pitch, roll) {
    const y = yaw * rad, p = pitch * rad, r = roll * rad;
    const f = [Math.sin(y)*Math.cos(p), Math.cos(y)*Math.cos(p), Math.sin(p)];
    const right = [Math.cos(y), -Math.sin(y), 0], up = cross(right, f);
    return [add(mul(right, Math.cos(r)), mul(up, -Math.sin(r))), add(mul(up, Math.cos(r)), mul(right, Math.sin(r))), f];
}
function project(p, head) {
    const z = dot(p, head[2]);
    if (z < 0.035) return null;
    return [600 + 1000*dot(p, head[0])/z, 360 - 1000*dot(p, head[1])/z];
}
function path(points, head, closed = false) {
    const projected = points.map(p => project(p, head));
    if (projected.some(p => !p)) return false;
    ctx.moveTo(...projected[0]);
    for (let i = 1; i < projected.length; i++) ctx.lineTo(...projected[i]);
    if (closed) ctx.closePath();
    return true;
}
function plane(p, frame) {
    return add(frame[2], add(mul(frame[0], (p[0]-600)/1000), mul(frame[1], (300-p[1])/1000)));
}
function instrument(indices, frames, head) {
    for (const index of indices) {
        const s = shapes[index], frame = frames[s.reference];
        ctx.beginPath();
        if (s.glyphs) {
            for (const quad of s.glyphs) path(quad.map(p => plane(p, frame)), head, true);
        } else if (!path(s.points.map(p => plane(p, frame)), head, s.fill)) continue;
        ctx.strokeStyle = 'rgba(0,8,10,.9)';
        ctx.lineWidth = s.glyphs ? 2.5 : s.width + 3;
        ctx.lineJoin = 'round'; ctx.lineCap = 'round'; ctx.stroke();
        const color = `rgba(${s.color[0]},${s.color[1]},${s.color[2]},${s.color[3]/255})`;
        if (s.glyphs || s.fill) { ctx.fillStyle = color; ctx.fill(); }
        else { ctx.strokeStyle = color; ctx.lineWidth = s.width; ctx.stroke(); }
    }
}
function backdrop(c) {
    ctx.fillStyle = c.background === 'night' ? '#030810' : c.background === 'snow' ? '#d3e3e9' : '#789ba9';
    ctx.fillRect(0, 0, 1200, 720);
    ctx.save(); ctx.translate(600, 360); ctx.rotate(-Number(inputs[2].value)*rad); ctx.translate(-600, -360);
    const horizon = 360 + Math.tan(Number(inputs[1].value)*rad)*1000;
    ctx.fillStyle = c.background === 'night' ? '#131e21' : c.background === 'snow' ? '#e6e6d6' : '#354943';
    ctx.fillRect(-4000, horizon, 9200, 15000);
    ctx.strokeStyle = c.background === 'snow' ? '#bbbfb3' : '#4b6258'; ctx.lineWidth = 1;
    for (let i = 1; i <= 16; i++) {
        const y = horizon + 1400/(i+1);
        ctx.beginPath(); ctx.moveTo(-3000, y); ctx.lineTo(4000, y); ctx.stroke();
    }
    ctx.restore();
}
function reviewZones(profile) {
    if (!document.querySelector('#zones').checked) return;
    ctx.save(); ctx.font = '11px system-ui'; ctx.lineWidth = 1; ctx.setLineDash([4, 5]);
    for (const zone of zones) {
        const [x, y, width, height] = zone.bounds;
        ctx.strokeStyle = '#d6dde0'; ctx.strokeRect(x, y+60, width, height);
        ctx.fillStyle = '#edf3f3'; ctx.fillText(zone.label, x+5, y+height+55);
    }
    ctx.setLineDash([]); ctx.fillStyle = '#edf3f3';
    ctx.fillText(profile.mission ? 'MISSION RESERVATION AVAILABLE' : 'MISSION CONTENT SUPPRESSED', 900, 670);
    ctx.restore();
}
function draw(now = performance.now()) {
    const c = cases[current], profile = c.profiles[Number(document.querySelector('#profile').value) || 0];
    const angles = inputs.map(x => Number(x.value)), head = basis(...angles);
    const separation = c.kind ? Math.acos(Math.max(-1, Math.min(1, dot(c.basis[2], head[2]))))/rad : 180;
    compact = c.kind === 0 || c.recovery || separation > (compact ? __EXIT__ : __ENTRY__);
    const frames = [c.basis, basis(angles[0], angles[1], 0), head];
    ctx.globalAlpha = 1; backdrop(c);
    ctx.globalAlpha = Number(document.querySelector('#brightness').value);
    if (!profile.blank) {
        ctx.beginPath(); for (const line of c.world) path(line, head);
        ctx.strokeStyle = 'rgba(0,8,10,.9)'; ctx.lineWidth = 4; ctx.stroke();
        ctx.strokeStyle = 'rgb(90,255,130)'; ctx.lineWidth = 1.6; ctx.stroke();
    }
    const view = compact ? profile.compact : profile.forward;
    instrument(view.steady, frames, head);
    if (now - eventStart < c.attentionMs) instrument(view.attention, frames, head);
    ctx.globalAlpha = 1; reviewZones(profile);
    ctx.fillStyle = '#efc564'; ctx.font = '12px system-ui';
    ctx.fillText('SYNTHETIC BENCH · '+c.name.toUpperCase(), 28, 30);
    const mode = c.recovery ? 'RECOVERY · flight priority' : profile.blank ? 'DUPLICATE FLIGHT CUES BLANKED · status and alerts retained' : compact ? 'OFF-AXIS · measured aircraft attitude' : 'FORWARD · display-fixed instruments';
    document.querySelector('#mode').textContent = mode + ' · flight text fixed to display';
    inputs.forEach((x, i) => document.getElementById(x.id+'Value').textContent = Math.round(angles[i])+'°');
}
function resetView() {
    compact = false; cases[current].head.forEach((v, i) => inputs[i].value = v); draw();
}
function replayModeEvent() {
    eventStart = performance.now();
    clearTimeout(eventTimer);
    eventTimer = setTimeout(() => draw(), cases[current].attentionMs + 1);
    draw();
}
function select(index) {
    current = index;
    document.querySelector('#casePicker').value = String(index);
    document.querySelector('#title').textContent = cases[index].name;
    document.querySelector('#note').textContent = cases[index].note;
    document.querySelectorAll('nav button').forEach((b, i) => b.classList.toggle('active', index === i));
    location.hash = String(index); resetView(); replayModeEvent();
}
profiles.forEach((name, i) => {
    const option = document.createElement('option'); option.value = String(i); option.textContent = name;
    document.querySelector('#profile').append(option);
});
cases.forEach((c, i) => {
    const button = document.createElement('button'); button.textContent = c.name; button.onclick = () => select(i);
    document.querySelector('nav').append(button);
    const option = document.createElement('option'); option.value = String(i); option.textContent = c.name;
    document.querySelector('#casePicker').append(option);
});
document.querySelector('#casePicker').onchange = e => select(Number(e.target.value));
for (const input of [...inputs, ...['brightness', 'profile', 'zones'].map(id => document.getElementById(id))]) input.oninput = () => draw();
document.querySelector('#reset').onclick = resetView;
document.querySelector('#replay').onclick = replayModeEvent;
document.querySelector('#save').onclick = () => {
    const a = document.createElement('a');
    a.download = 'indicate-hwd-'+cases[current].name.replaceAll(' ', '-')+'.png';
    a.href = canvas.toDataURL(); a.click();
};
document.querySelector('#sweep').onclick = () => {
    running = !running; document.querySelector('#sweep').textContent = running ? 'Stop sweep' : 'Sweep head';
};
function animate(t) {
    if (t-last > 50) {
        if (running) {
            inputs[0].value = (cases[current].head[0]+Math.sin(t/2300)*70+360)%360;
            inputs[1].value = cases[current].head[1]+Math.sin(t/3900)*20;
        }
        if (running || t-eventStart <= cases[current].attentionMs+100) draw(t);
        last = t;
    }
    requestAnimationFrame(animate);
}
requestAnimationFrame(animate);
select(Math.min(cases.length-1, Math.max(0, Number(location.hash.slice(1)) || 0)));
