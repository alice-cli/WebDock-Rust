// WebDock client — modular sections (single bundle for embedding).
// 1) Theme  2) Modal  3) Icons  4) WebSocket  5) Quick launch
// 6) Window/App lists + drag order  7) Stage input  8) Bootstrap

const cv = document.getElementById('cv'), ctx = cv.getContext('2d');
const statusEl = document.getElementById('statusText'), listEl = document.getElementById('list');
const dot = document.getElementById('dot'), fpsEl = document.getElementById('fps'), emptyEl = document.getElementById('empty');
let ws, mode = 'windows', activeId = null, winList = [], appList = [], winJSON = '', appQuery = '', winQuery = '', frames = 0, selW = 0, selH = 0;
let quick = JSON.parse(localStorage.getItem('webdock.quick') || '[]');
let winOrder = JSON.parse(localStorage.getItem('webdock.winOrder') || '[]'); // window id[]
let appOrder = JSON.parse(localStorage.getItem('webdock.appOrder') || '[]'); // path[]
let iconByName = {}, iconByPath = {}, pendingApp = null;
let imeKorean = false;
let quickEdit = false;
let modalResolve = null;
let listDidDrag = false;
// 원격 Cmd+C/X 후 이 기기 클립보드로 자동 가져오기 (localStorage)
let clipAuto = localStorage.getItem('webdock.clipAuto') !== '0'; // default ON

function showConfirm({ title, body, okText, danger }){
  return new Promise(resolve => {
    const modal = document.getElementById('modal');
    document.getElementById('modalTitle').textContent = title || t('confirm');
    document.getElementById('modalBody').textContent = body || '';
    const ok = document.getElementById('modalOk');
    ok.textContent = okText || t('confirm');
    ok.className = 'btn' + (danger === false ? '' : ' danger');
    const cancel = document.getElementById('modalCancel');
    if (cancel) cancel.textContent = t('cancel');
    modalResolve = resolve;
    modal.classList.add('open');
    ok.focus();
  });
}
function closeModal(result){
  document.getElementById('modal').classList.remove('open');
  const r = modalResolve;
  modalResolve = null;
  if (r) r(!!result);
}
document.getElementById('modalCancel').onclick = () => closeModal(false);
document.getElementById('modalOk').onclick = () => closeModal(true);
document.getElementById('modal').addEventListener('click', e => {
  if (e.target.id === 'modal') closeModal(false);
});
document.addEventListener('keydown', e => {
  if (!document.getElementById('modal').classList.contains('open')) return;
  if (e.key === 'Escape') { e.preventDefault(); closeModal(false); }
  if (e.key === 'Enter') { e.preventDefault(); closeModal(true); }
});

function toggleQuickEdit(){
  quickEdit = !quickEdit;
  document.body.classList.toggle('quick-edit', quickEdit);
  const btn = document.getElementById('quickEditBtn');
  if (btn) {
    btn.classList.toggle('on', quickEdit);
    btn.textContent = quickEdit ? t('done') : t('edit');
  }
  if (!quickEdit) renderQuick();
}

// ---- 테마 (라이트 / 다크) ----
function preferredTheme(){
  const saved = localStorage.getItem('webdock.theme');
  if (saved === 'light' || saved === 'dark') return saved;
  return window.matchMedia && window.matchMedia('(prefers-color-scheme: light)').matches ? 'light' : 'dark';
}
function applyTheme(theme){
  const light = theme === 'light';
  document.documentElement.classList.toggle('light', light);
  localStorage.setItem('webdock.theme', light ? 'light' : 'dark');
  const btn = document.getElementById('themeBtn');
  if (btn) btn.title = light ? t('themeDark') : t('themeLight');
}
function toggleTheme(){
  applyTheme(document.documentElement.classList.contains('light') ? 'dark' : 'light');
}
applyTheme(preferredTheme());
if (window.matchMedia) {
  window.matchMedia('(prefers-color-scheme: light)').addEventListener('change', e => {
    if (!localStorage.getItem('webdock.theme')) applyTheme(e.matches ? 'light' : 'dark');
  });
}

// Auth token from URL (?token=), sessionStorage, or cookie — required when server locks access.
function cookieToken(){
  try {
    const m = document.cookie.match(/(?:^|;\\s*)webdock_token=([^;]*)/);
    return m ? decodeURIComponent(m[1]) : '';
  } catch (_) { return ''; }
}
function clearStoredToken(){
  try { sessionStorage.removeItem('webdock.token'); } catch (_) {}
  try { document.cookie = 'webdock_token=; Path=/; Max-Age=0; SameSite=Strict'; } catch (_) {}
}
function resolveToken(){
  try {
    // Only the first `token` query value — never join multiple params.
    const params = new URLSearchParams(location.search);
    const q = params.get('token');
    if (q) {
      // Replace any previous stored token (do not append).
      sessionStorage.setItem('webdock.token', q);
      try {
        const u = new URL(location.href);
        u.searchParams.delete('token');
        history.replaceState(null, '', u.pathname + u.search + u.hash);
      } catch (_) {}
      return q;
    }
    // Prefer cookie over sessionStorage (server is source of truth after POST login).
    const c = cookieToken();
    if (c) {
      sessionStorage.setItem('webdock.token', c);
      return c;
    }
    return sessionStorage.getItem('webdock.token') || '';
  } catch (_) { return ''; }
}
function wsURL(){
  const base = (location.protocol==='https:'?'wss':'ws') + '://' + location.host + '/ws';
  const t = resolveToken();
  return t ? base + '?token=' + encodeURIComponent(t) : base;
}
function send(o){ if(ws && ws.readyState===1) ws.send(JSON.stringify(o)); }

function applyIMEState(korean){
  const next = !!korean;
  if (imeKorean && !next) hangulFlush(false);
  imeKorean = next;
  const btn = document.getElementById('imeBtn');
  if (!btn) return;
  btn.classList.toggle('ko', imeKorean);
  btn.classList.toggle('en', !imeKorean);
  btn.title = imeKorean ? t('imeKo') : t('imeEn');
}
function flashIME(){
  const btn = document.getElementById('imeBtn');
  if (!btn) return;
  btn.classList.add('flash');
  setTimeout(() => btn.classList.remove('flash'), 220);
}
function toggleIME(){
  flashIME();
  hangulFlush(false);
  const next = !imeKorean;
  applyIMEState(next);
  // Absolute state — server sets Mac IME to match (no toggle race).
  send({type:'ime', korean: next});
  cv.focus();
}

function rememberIcon(item){
  if (!item || !item.icon) return;
  if (item.name) iconByName[item.name] = item.icon;
  if (item.path) iconByPath[item.path] = item.icon;
}

function connect(){
  ws = new WebSocket(wsURL());
  ws.binaryType = 'arraybuffer';
  ws.onopen = () => {
    dot.classList.add('on');
    statusEl.textContent = t('connected');
    send({type:'apps'});
    send({type:'refresh'});
    send({type:'imeState'});
    send({type:'clipAuto', value: clipAuto});
    syncClipAutoBtn();
  };
  ws.onclose = (ev) => {
    dot.classList.remove('on');
    // 403/auth failures: do not hammer reconnect with a bad token.
    if (ev.code === 1008 || ev.code === 1002 || ev.code === 1003) {
      clearStoredToken();
      statusEl.textContent = t('authFail');
      // Hard reload gate so user can re-enter a clean token (no accumulation).
      setTimeout(() => { location.href = location.pathname; }, 600);
      return;
    }
    statusEl.textContent = t('disconnected');
    setTimeout(connect, 1000);
  };
  ws.onerror = () => statusEl.textContent = t('connError');
  ws.onmessage = async (ev) => {
    if (typeof ev.data === 'string') {
      const m = JSON.parse(ev.data);
      if (m.type === 'windows') {
        let iconsChanged = false;
        const slim = m.list.map(w => {
          if (w.icon) {
            const before = resolveIcon(w.name, w.path);
            rememberIcon(w);
            if (before !== w.icon) iconsChanged = true;
          }
          const {icon, ...rest} = w;
          return rest;
        });
        const j = JSON.stringify(slim);
        const listChanged = j !== winJSON;
        if (listChanged) {
          winJSON = j;
          winList = slim;
          const s0 = winList.find(x => x.id===activeId); if (s0) { selW = s0.w; selH = s0.h; }
          if (pendingApp) {
            const w = winList.find(x => x.name===pendingApp || (x.path && quick.some(q => q.path===x.path && q.name===pendingApp)));
            if (w) { pendingApp = null; selectWindow(w); }
          }
          updateBadges();
        }
        if ((listChanged || iconsChanged) && mode==='windows') {
          if (document.getElementById('winItems')) renderItemsOnly();
          else render();
        }
      } else if (m.type === 'apps') {
        appList = m.list;
        appList.forEach(rememberIcon);
        updateBadges();
        renderQuick();
        if (mode==='apps') renderAppItems();
        else if (mode==='windows' && document.getElementById('winItems')) renderItemsOnly();
        else if (mode==='windows') render();
      } else if (m.type === 'ime') {
        applyIMEState(!!m.korean);
      } else if (m.type === 'clients') {
        renderClients(m.list || []);
      } else if (m.type === 'metrics') {
        applyMetrics(m);
      } else if (m.type === 'clipboard') {
        // force=true: 수동 "가져오기" — 자동 OFF여도 적용
        if (clipAuto || m.force) applyRemoteClipboard(m);
      } else if (m.type === 'inputBusy') {
        statusEl.textContent = m.message || t('busy');
      } else if (m.type === 'h264config') {
        setupH264Decoder(m);
      }
    } else {
      const buf = ev.data;
      const u8 = new Uint8Array(buf, 0, Math.min(16, buf.byteLength || 0));
      // H.264 sample packet (type byte 0x01)
      if (u8.length >= 14 && u8[0] === 0x01) {
        handleH264Sample(new Uint8Array(buf));
        return;
      }
      // Sniff PNG (89 50 4E 47) vs JPEG (FF D8)
      const isPng = u8.length >= 4 && u8[0] === 0x89 && u8[1] === 0x50 && u8[2] === 0x4E && u8[3] === 0x47;
      const mime = isPng ? 'image/png' : 'image/jpeg';
      const bmp = await createImageBitmap(new Blob([buf], {type: mime}));
      const resized = (cv.width !== bmp.width || cv.height !== bmp.height);
      if (resized) {
        cv.width = bmp.width;
        cv.height = bmp.height;
      }
      ctx.imageSmoothingEnabled = true;
      ctx.imageSmoothingQuality = 'high';
      ctx.drawImage(bmp, 0, 0); bmp.close();
      cv.style.display = 'block'; emptyEl.style.display = 'none'; frames++;
      if (resized || frames <= 2 || frames % 10 === 0) fitCanvas();
    }
  };
}
setInterval(() => { fpsEl.textContent = frames + ' fps'; frames = 0; }, 1000);

/**
 * 뷰포트: 모바일은 "가로 맞춤 확대"로 글자가 읽히게 하고,
 * 두 손가락 핀치/팬으로 추가 확대·이동. 데스크톱은 전체 맞춤.
 * view.zoom = 1 → baseScale() 기준 (모바일=가로채움, PC=전체넣기)
 */
const view = { zoom: 1, panX: 0, panY: 0, mode: 'auto' }; // mode: auto | free

function stageSize(){
  const stage = document.getElementById('stage');
  if (!stage) return { sw: 0, sh: 0, stage: null };
  return { sw: stage.clientWidth, sh: stage.clientHeight, stage };
}

function baseScale(){
  const { sw, sh } = stageSize();
  if (!cv.width || !cv.height || sw < 2 || sh < 2) return 1;
  // 모바일은 원격 창 해상도를 기기 뷰포트에 맞춘 뒤, 여기선 양축 contain → 꽉 참
  return Math.min(sw / cv.width, sh / cv.height);
}

/** 디스플레이 캡처 라우트(전체 화면) 여부 — 창 리사이즈 대상 아님 */
function isDisplayRouteId(id){
  if (id == null) return false;
  const n = Number(id) >>> 0;
  return n >= 0xE0000000;
}

/**
 * 모바일: 원격 Mac 창 크기를 기기 화면 영역과 같게 맞춤
 * (= 모니터 해상도 바꾸는 것과 같은 효과 → 글자·UI가 모바일에 맞게 재배치)
 */
let _resTimer = null;
let _lastRes = { w: 0, h: 0 };

function scheduleMobileResolution(){
  if (!isMobileLayout()) return;
  if (activeId == null || isDisplayRouteId(activeId)) return;
  clearTimeout(_resTimer);
  _resTimer = setTimeout(applyMobileResolution, 300);
}

function applyMobileResolution(){
  if (!isMobileLayout()) return;
  if (activeId == null || isDisplayRouteId(activeId)) return;
  const { sw, sh } = stageSize();
  if (sw < 120 || sh < 120) return;

  // CSS 픽셀 = 원격 창 포인트 크기 (레티나 배수 없이 → UI가 폰에 맞게 큼)
  let w = Math.round(sw);
  let h = Math.round(sh);
  w = Math.max(320, Math.min(1280, w));
  h = Math.max(400, Math.min(1600, h));

  if (Math.abs(w - _lastRes.w) < 10 && Math.abs(h - _lastRes.h) < 10) {
    fitCanvas();
    return;
  }
  _lastRes = { w, h };
  selW = w; selH = h;
  send({ type: 'resize', w, h });
  view.mode = 'auto';
  view.zoom = 1;
  view.panX = 0;
  view.panY = 0;
  statusEl.textContent = t('resFit') + ' ' + w + '×' + h;
  // 캡처 재시작 후 프레임 오면 꽉 참
  setTimeout(fitCanvas, 500);
}

function clampPan(){
  const { sw, sh } = stageSize();
  const z = baseScale() * view.zoom;
  const w = cv.width * z;
  const h = cv.height * z;
  if (w <= sw) view.panX = (sw - w) / 2;
  else view.panX = Math.min(0, Math.max(sw - w, view.panX));
  if (h <= sh) view.panY = (sh - h) / 2;
  else view.panY = Math.min(0, Math.max(sh - h, view.panY));
}

function applyView(){
  if (!cv.width || !cv.height) return;
  const z = baseScale() * view.zoom;
  const w = Math.max(1, Math.round(cv.width * z));
  const h = Math.max(1, Math.round(cv.height * z));
  clampPan();
  cv.style.position = 'absolute';
  cv.style.left = Math.round(view.panX) + 'px';
  cv.style.top = Math.round(view.panY) + 'px';
  cv.style.width = w + 'px';
  cv.style.height = h + 'px';
  cv.style.maxWidth = 'none';
  cv.style.maxHeight = 'none';
}

function fitCanvas(){
  // 레이아웃 변경 시 호출. auto 모드면 줌 리셋 + 모바일 해상도 동기화 트리거
  if (view.mode === 'auto') {
    view.zoom = 1;
    view.panX = 0;
    view.panY = 0;
  }
  applyView();
  updateZoomHint();
}

function setZoom(z, cx, cy){
  // cx,cy = stage 기준 줌 중심 (있으면 그 점 고정)
  const { stage } = stageSize();
  if (!stage || !cv.width) return;
  const oldAbs = baseScale() * view.zoom;
  view.zoom = Math.max(0.5, Math.min(5, z));
  const newAbs = baseScale() * view.zoom;
  if (cx != null && cy != null && oldAbs > 0) {
    const contentX = (cx - view.panX) / oldAbs;
    const contentY = (cy - view.panY) / oldAbs;
    view.panX = cx - contentX * newAbs;
    view.panY = cy - contentY * newAbs;
  }
  view.mode = 'free';
  applyView();
  updateZoomHint();
}

function resetViewAuto(){
  view.mode = 'auto';
  view.zoom = 1;
  view.panX = 0;
  view.panY = 0;
  applyView();
  updateZoomHint();
  if (isMobileLayout()) {
    scheduleMobileResolution();
    statusEl.textContent = t('fitDevice');
  } else {
    statusEl.textContent = t('fitAll');
  }
}

function updateZoomHint(){
  const el = document.getElementById('zoomHint');
  if (!el) return;
  el.textContent = Math.round(view.zoom * 100) + '%';
}
function zoomInStep(){
  const { sw, sh } = stageSize();
  setZoom(view.zoom * 1.25, sw / 2, sh / 2);
}
function zoomOutStep(){
  const { sw, sh } = stageSize();
  setZoom(view.zoom / 1.25, sw / 2, sh / 2);
}

let _stageRO = null;
function watchStageSize(){
  const stage = document.getElementById('stage');
  if (!stage || typeof ResizeObserver === 'undefined') return;
  if (_stageRO) _stageRO.disconnect();
  _stageRO = new ResizeObserver(() => {
    fitCanvas();
    scheduleMobileResolution();
  });
  _stageRO.observe(stage);
}
window.addEventListener('resize', () => {
  fitCanvas();
  scheduleMobileResolution();
});
window.addEventListener('orientationchange', () => {
  setTimeout(() => {
    fitCanvas();
    scheduleMobileResolution();
  }, 200);
});
watchStageSize();

function esc(s){ const d = document.createElement('div'); d.textContent = s||''; return d.innerHTML; }
function hashCode(s){ let h=0; for(let i=0;i<(s||'').length;i++) h=(h*31+s.charCodeAt(i))|0; return Math.abs(h); }

function resolveIcon(name, path){
  if (path && iconByPath[path]) return iconByPath[path];
  if (name && iconByName[name]) return iconByName[name];
  return null;
}

function iconHTML(name, path){
  const src = resolveIcon(name, path);
  if (src) return '<img class="ava" src="'+src+'" alt="" draggable="false">';
  const c = ['#e55','#3c9','#4f8cff','#a78bfa','#f59e0b','#14b8a6'][hashCode(name)%6];
  return '<span class="ava" style="background:'+c+'">'+esc((name||'?')[0].toUpperCase())+'</span>';
}

function updateBadges(){
  const wb = document.getElementById('winBadge');
  const ab = document.getElementById('appBadge');
  const qc = document.getElementById('quickCount');
  if (wb) wb.textContent = String(winList.length);
  if (ab) ab.textContent = String(appList.length);
  if (qc) qc.textContent = String(quick.length);
}

function saveQuick(){ localStorage.setItem('webdock.quick', JSON.stringify(quick)); }
function pinApp(a){
  if (!a || !a.path) return;
  if (!quick.find(q => q.path===a.path)){
    quick.push({name:a.name, path:a.path});
    saveQuick(); renderQuick(); updateBadges();
    statusEl.textContent = t('pinned') + ': ' + a.name;
  }
}
function pinFromWindow(w){
  if (w.path) pinApp({name:w.name, path:w.path});
  else {
    const match = appList.find(a => a.name === w.name);
    if (match) pinApp(match);
    else statusEl.textContent = t('pinFail') + ': ' + w.name;
  }
}

function renderQuick(){
  const q = document.getElementById('quickList');
  if (!q) return;
  q.innerHTML = '';
  updateBadges();
  if (!quick.length){
    q.innerHTML = '<div class="qhint">'+t('qhint')+(quickEdit ? '' : t('qhintEdit'))+'</div>';
    return;
  }
  quick.forEach((item) => {
    const el = document.createElement('button');
    el.type = 'button';
    el.className = 'chip';
    el.title = item.name;
    el.setAttribute('aria-label', item.name);
    el.innerHTML = iconHTML(item.name, item.path);
    el.onclick = () => {
      if (quickEdit) return;
      activateQuick(item);
    };
    const x = document.createElement('button');
    x.type = 'button';
    x.className = 'chip-x';
    x.textContent = '×';
    x.title = t('unpin');
    x.onclick = async (e) => {
      e.stopPropagation();
      e.preventDefault();
      const ok = await showConfirm({
        title: t('unpin'),
        body: item.name + ' — ' + t('unpinBody'),
        okText: t('remove'),
        danger: true
      });
      if (!ok) return;
      const i = quick.findIndex(q => q.path === item.path);
      if (i >= 0) { quick.splice(i, 1); saveQuick(); renderQuick(); }
    };
    el.appendChild(x); q.appendChild(el);
  });
}
function activateQuick(item){
  const win = winList.find(w => w.name===item.name || (item.path && w.path===item.path));
  if (win) selectWindow(win);
  else {
    launchAppPath(item.path, item.name, false);
  }
}
/** Launch .app; newInstance=true → new window (same process), not a second Terminal instance. */
let _launchBusyUntil = 0;
function launchAppPath(path, name, newInstance){
  if (!path) return;
  const now = performance.now();
  if (now < _launchBusyUntil) return; // debounce double-clicks opening 2 windows
  _launchBusyUntil = now + 800;
  send({type:'launch', path:path, newInstance: !!newInstance});
  pendingApp = name || null;
  statusEl.textContent = (name || 'app') + ' — ' + (newInstance ? t('newInstance') : t('launching'));
  if (isMobileLayout()) closeSidebar();
}
// 호버 시 상세 (body 고정 레이어 — 사이드바 overflow에 안 잘림)
let metricTipEl = null;
function ensureMetricTip(){
  if (metricTipEl) return metricTipEl;
  metricTipEl = document.createElement('div');
  metricTipEl.id = 'metricTip';
  document.body.appendChild(metricTipEl);
  return metricTipEl;
}
function showMetricTip(el, text){
  const tip = ensureMetricTip();
  tip.textContent = text || '';
  if (!text) { tip.classList.remove('show'); return; }
  const r = el.getBoundingClientRect();
  tip.classList.add('show');
  const tw = tip.offsetWidth || 80;
  const th = tip.offsetHeight || 28;
  let left = r.left + r.width / 2 - tw / 2;
  let top = r.bottom + 8;
  left = Math.max(6, Math.min(left, window.innerWidth - tw - 6));
  if (top + th > window.innerHeight - 6) top = r.top - th - 8;
  tip.style.left = left + 'px';
  tip.style.top = top + 'px';
}
function hideMetricTip(){
  if (metricTipEl) metricTipEl.classList.remove('show');
}
function bindMetricHover(el){
  if (!el || el._metricBound) return;
  el._metricBound = true;
  el.addEventListener('mouseenter', () => showMetricTip(el, el.getAttribute('data-tip') || ''));
  el.addEventListener('mouseleave', hideMetricTip);
  el.addEventListener('mousemove', () => {
    if (metricTipEl && metricTipEl.classList.contains('show'))
      showMetricTip(el, el.getAttribute('data-tip') || '');
  });
}

function setMetricVal(id, text, pct){
  const el = document.getElementById(id);
  if (!el) return;
  el.textContent = text;
  el.classList.remove('warn', 'bad');
  const v = Math.max(0, Math.min(100, Number(pct) || 0));
  if (v >= 90) el.classList.add('bad');
  else if (v >= 70) el.classList.add('warn');
}

function fmtGB(n){
  const v = Number(n);
  if (!isFinite(v)) return '?';
  if (v >= 100) return Math.round(v) + 'G';
  if (v >= 10) return v.toFixed(1) + 'G';
  return v.toFixed(2) + 'G';
}

function applyMetrics(m){
  const cpu = Number(m.cpu ?? 0);
  const ram = Number(m.ram ?? 0);
  const disk = Number(m.disk ?? 0);
  setMetricVal('mCpuVal', cpu.toFixed(0) + '%', cpu);
  setMetricVal('mRamVal', fmtGB(m.ramUsedGB), ram);
  setMetricVal('mDiskVal', fmtGB(m.diskUsedGB), disk);
  const cpuEl = document.getElementById('mCpu');
  const ramEl = document.getElementById('mRam');
  const diskEl = document.getElementById('mDisk');
  if (cpuEl) {
    cpuEl.setAttribute('data-tip', 'CPU  ' + cpu.toFixed(1) + '%');
    bindMetricHover(cpuEl);
  }
  if (ramEl) {
    ramEl.setAttribute('data-tip',
      'RAM  ' + ram.toFixed(1) + '% · ' + (m.ramUsedGB ?? '?') + ' / ' + (m.ramTotalGB ?? '?') + ' GB');
    bindMetricHover(ramEl);
  }
  if (diskEl) {
    diskEl.setAttribute('data-tip',
      t('diskLine') + '  ' + disk.toFixed(1) + '% · ' + (m.diskUsedGB ?? '?') + ' / ' + (m.diskTotalGB ?? '?') + ' GB');
    bindMetricHover(diskEl);
  }
}

function renderClients(list){
  window._lastClients = list || [];
  const box = document.getElementById('clientList');
  const cnt = document.getElementById('clientCount');
  if (cnt) cnt.textContent = String((list || []).length);
  if (!box) return;
  box.innerHTML = '';
  if (!list || !list.length){
    box.innerHTML = '<div class="client-empty">'+t('noClients')+'</div>';
    return;
  }
  list.forEach(c => {
    const row = document.createElement('div');
    row.className = 'client-row';
    const ip = esc(c.ip || '?');
    const app = (c.app || '').trim();
    const title = (c.title || '').trim();
    let view = t('noView');
    if (app) view = app + (title ? ' — ' + title : '');
    row.innerHTML = '<span class="cip">'+ip+'</span><span class="cview" title="'+esc(view)+'">'+esc(view)+'</span>';
    box.appendChild(row);
  });
}

function selectWindow(w){
  activeId = w.id; selW = w.w; selH = w.h;
  _lastRes = { w: 0, h: 0 }; // 새 창이면 해상도 다시 맞춤
  if (mode==='windows') render();
  // H.264: fully reset decoder so the next h264config+key from the new stream
  // is applied. Keeping the old decoder caused freeze / black on window switch.
  if (streamFormat === 'h264') {
    teardownH264();
    h264WaitingKey = true;
    _h264StatsGraceUntil = performance.now() + 4000;
    _statFrames = 0;
    _statDrops = 0;
    // Clear last frame so user sees we're switching (not a frozen old window).
    try {
      ctx.clearRect(0, 0, cv.width || 2, cv.height || 2);
    } catch (_) {}
  }
  send({type:'select', id:w.id});
  if (streamFormat === 'h264') {
    send({type:'keyframe', id:w.id});
  }
  statusEl.textContent = t('streaming')+': '+w.name+(w.title ? ' — '+w.title : '');
  cv.focus();
  // 모바일: 창 고르면 메뉴 닫고 → 원격 창을 폰 해상도로 맞춤
  if (isMobileLayout()) {
    closeSidebar();
    scheduleMobileResolution();
  } else {
    fitCanvas();
  }
}

function setMode(m){
  mode = m;
  document.getElementById('tabWin').classList.toggle('active', m==='windows');
  document.getElementById('tabApp').classList.toggle('active', m==='apps');
  if (m==='apps') { send({type:'apps'}); buildAppsUI(); } else { send({type:'refresh'}); render(); }
}

function refreshList(){
  if (mode==='apps') send({type:'apps'});
  else send({type:'refresh'});
  statusEl.textContent = t('refreshList');
}

// ---- 목록 순서 (드래그 앤 드롭) ----
function saveWinOrder(){ localStorage.setItem('webdock.winOrder', JSON.stringify(winOrder)); }
function saveAppOrder(){ localStorage.setItem('webdock.appOrder', JSON.stringify(appOrder)); }

function orderedByKeys(list, order, keyFn){
  const map = new Map(list.map(item => [String(keyFn(item)), item]));
  const out = [];
  for (const k of order) {
    const s = String(k);
    if (map.has(s)) { out.push(map.get(s)); map.delete(s); }
  }
  for (const item of list) {
    const s = String(keyFn(item));
    if (map.has(s)) { out.push(item); map.delete(s); }
  }
  return out;
}

function mergeVisibleOrder(fullOrder, visibleNewOrder){
  // fullOrder / visibleNewOrder → string[]
  const visNew = visibleNewOrder.map(String);
  const vis = new Set(visNew);
  let i = 0;
  const next = fullOrder.map(k => {
    const s = String(k);
    return vis.has(s) ? visNew[i++] : s;
  });
  for (; i < visNew.length; i++) {
    if (!next.includes(visNew[i])) next.push(visNew[i]);
  }
  return next;
}

function bindListDrag(container, { onReorder }){
  let dragEl = null;
  container.querySelectorAll('.item[data-key]').forEach(el => {
    el.draggable = true;
    el.addEventListener('dragstart', e => {
      if (e.target.closest && e.target.closest('.act')) { e.preventDefault(); return; }
      dragEl = el;
      listDidDrag = false;
      el.classList.add('dragging');
      e.dataTransfer.effectAllowed = 'move';
      try { e.dataTransfer.setData('text/plain', el.dataset.key); } catch(_) {}
    });
    el.addEventListener('dragend', () => {
      el.classList.remove('dragging');
      container.querySelectorAll('.drag-over-before,.drag-over-after').forEach(n => {
        n.classList.remove('drag-over-before', 'drag-over-after');
      });
      dragEl = null;
    });
    el.addEventListener('dragover', e => {
      e.preventDefault();
      e.dataTransfer.dropEffect = 'move';
      if (!dragEl || dragEl === el) return;
      const rect = el.getBoundingClientRect();
      const before = e.clientY < rect.top + rect.height / 2;
      container.querySelectorAll('.item').forEach(n => n.classList.remove('drag-over-before','drag-over-after'));
      el.classList.add(before ? 'drag-over-before' : 'drag-over-after');
    });
    el.addEventListener('dragleave', () => {
      el.classList.remove('drag-over-before', 'drag-over-after');
    });
    el.addEventListener('drop', e => {
      e.preventDefault();
      e.stopPropagation();
      if (!dragEl || dragEl === el) return;
      listDidDrag = true;
      const rect = el.getBoundingClientRect();
      const before = e.clientY < rect.top + rect.height / 2;
      if (before) container.insertBefore(dragEl, el);
      else container.insertBefore(dragEl, el.nextSibling);
      container.querySelectorAll('.drag-over-before,.drag-over-after').forEach(n => {
        n.classList.remove('drag-over-before', 'drag-over-after');
      });
      const keys = [...container.querySelectorAll('.item[data-key]')].map(n => n.dataset.key);
      onReorder(keys);
    });
  });
}

function filteredWindows(){
  const ordered = orderedByKeys(winList, winOrder, w => w.id);
  const q = winQuery.trim().toLowerCase();
  if (!q) return ordered;
  return ordered.filter(w =>
    (w.name||'').toLowerCase().includes(q) ||
    (w.title||'').toLowerCase().includes(q)
  );
}

function filteredApps(){
  const ordered = orderedByKeys(appList, appOrder, a => a.path);
  const q = appQuery.trim().toLowerCase();
  if (!q) return ordered;
  return ordered.filter(a => (a.name||'').toLowerCase().includes(q));
}

function render(){
  if (mode !== 'windows') return;
  listEl.innerHTML = '';
  const wrap = document.createElement('div'); wrap.className = 'search-wrap';
  const s = document.createElement('input');
  s.className = 'search'; s.id = 'winSearch'; s.placeholder = t('searchWin'); s.value = winQuery;
  s.addEventListener('input', () => { winQuery = s.value; renderItemsOnly(); });
  wrap.appendChild(s); listEl.appendChild(wrap);
  const items = document.createElement('div'); items.id = 'winItems'; listEl.appendChild(items);
  renderItemsOnly();
}

function renderItemsOnly(){
  const items = document.getElementById('winItems'); if (!items) return;
  const filtered = filteredWindows();
  items.innerHTML = '';
  if (!winList.length){
    items.innerHTML = '<div class="hint">'+t('noWindows')+'</div>';
    return;
  }
  if (!filtered.length){
    items.innerHTML = '<div class="hint">'+t('noSearch')+'</div>';
    return;
  }
  filtered.forEach(w => {
    const el = document.createElement('button');
    el.type = 'button';
    el.className = 'item' + (w.id===activeId ? ' active' : '');
    el.dataset.key = String(w.id);
    const live = w.id===activeId ? '<span class="live-dot" title="'+t('live')+'"></span>' : '';
    const size = (w.w && w.h) ? '<span class="meta">'+w.w+'×'+w.h+'</span>' : '';
    const isDisplay = isDisplayRouteId(w.id);
    el.innerHTML = '<span class="drag-handle" title="'+t('dragReorder')+'">⠿</span>'
      + iconHTML(w.name, w.path)
      + '<span class="info"><span class="name">'+esc(w.name)+'</span><span class="title">'+esc(w.title||t('noTitle'))+'</span>'+size+'</span>'
      + live;
    const pin = document.createElement('button'); pin.type = 'button'; pin.className = 'act q'; pin.textContent = '☆'; pin.title = t('pinQuick');
    pin.draggable = false;
    pin.onclick = (ev) => { ev.stopPropagation(); pinFromWindow(w); };
    // 같은 앱 새 인스턴스 (Terminal 추가 창 등)
    if (w.path && !isDisplay) {
      const neu = document.createElement('button');
      neu.type = 'button';
      neu.className = 'act n';
      neu.textContent = '+';
      neu.title = w.name + ' — ' + t('newWin');
      neu.draggable = false;
      neu.onclick = (ev) => {
        ev.stopPropagation();
        launchAppPath(w.path, w.name, true);
        setTimeout(() => setMode('windows'), 900);
      };
      el.appendChild(neu);
    }
    el.appendChild(pin);
    if (!isDisplay && w.pid) {
      const x = document.createElement('button');
      x.type = 'button';
      x.className = 'act x';
      x.textContent = '×';
      x.title = t('closeWin');
      x.draggable = false;
      x.onclick = async (ev) => {
        ev.stopPropagation();
        const ok = await showConfirm({
          title: t('closeWinTitle'),
          body: (w.title ? '[' + w.title + '] ' : '') + w.name + t('closeWinBody'),
          okText: t('closeWinTitle'),
          danger: true
        });
        if (!ok) return;
        send({type:'close', id:w.id, pid:w.pid, title:w.title || ''});
        if (w.id === activeId) {
          activeId = null;
          cv.style.display = 'none';
          emptyEl.style.display = 'flex';
        }
        statusEl.textContent = t('closing');
      };
      el.appendChild(x);
    }
    el.onclick = () => {
      if (listDidDrag) { listDidDrag = false; return; }
      selectWindow(w);
    };
    items.appendChild(el);
  });
  bindListDrag(items, {
    onReorder(visibleKeys){
      const full = orderedByKeys(winList, winOrder, w => w.id).map(w => String(w.id));
      // full에 아직 없는 창 id도 포함
      for (const w of winList) {
        const k = String(w.id);
        if (!full.includes(k)) full.push(k);
      }
      winOrder = mergeVisibleOrder(full, visibleKeys).map(k => {
        const n = Number(k);
        return Number.isFinite(n) ? n : k;
      });
      // 죽은 id 정리
      const alive = new Set(winList.map(w => w.id));
      winOrder = winOrder.filter(id => alive.has(id) || alive.has(Number(id)));
      saveWinOrder();
    }
  });
}

function buildAppsUI(){
  listEl.innerHTML = '';
  const wrap = document.createElement('div'); wrap.className = 'search-wrap';
  const s = document.createElement('input');
  s.className = 'search'; s.id = 'appSearch'; s.placeholder = t('searchApp'); s.value = appQuery;
  s.addEventListener('input', () => { appQuery = s.value; renderAppItems(); });
  wrap.appendChild(s); listEl.appendChild(wrap);
  const items = document.createElement('div'); items.id = 'appItems';
  listEl.appendChild(items);
  if (!appList.length) items.innerHTML = '<div class="hint">'+t('loadingApps')+'</div>';
  else renderAppItems();
  requestAnimationFrame(() => s.focus());
}
function renderAppItems(){
  const items = document.getElementById('appItems'); if (!items) return;
  const filtered = filteredApps();
  items.innerHTML = '';
  if (!filtered.length){ items.innerHTML = '<div class="hint">'+t('noSearch')+'</div>'; return; }
  filtered.forEach(a => {
    const el = document.createElement('button');
    el.type = 'button';
    el.className = 'item';
    el.dataset.key = a.path;
    el.innerHTML = '<span class="drag-handle" title="'+t('dragReorder')+'">⠿</span>'
      + iconHTML(a.name, a.path)
      + '<span class="info"><span class="name">'+esc(a.name)+'</span></span>';
    const pin = document.createElement('button'); pin.type = 'button'; pin.className = 'act q'; pin.textContent = '☆'; pin.title = t('pinQuick');
    pin.draggable = false;
    pin.onclick = (ev) => { ev.stopPropagation(); pinApp(a); };
    // 이미 실행 중이어도 새 인스턴스
    const neu = document.createElement('button');
    neu.type = 'button';
    neu.className = 'act n';
    neu.textContent = '+';
    neu.title = a.name + ' — ' + t('newWin');
    neu.draggable = false;
    neu.onclick = (ev) => {
      ev.stopPropagation();
      launchAppPath(a.path, a.name, true);
      setTimeout(() => setMode('windows'), 900);
    };
    el.appendChild(neu);
    el.appendChild(pin);
    el.onclick = () => {
      if (listDidDrag) { listDidDrag = false; return; }
      // 기본 클릭: 기존 인스턴스 활성화(또는 최초 실행)
      const running = winList.some(w => w.path === a.path || w.name === a.name);
      launchAppPath(a.path, a.name, false);
      if (!running) setTimeout(() => setMode('windows'), 900);
      else {
        const win = winList.find(w => w.path === a.path || w.name === a.name);
        if (win) selectWindow(win);
        setMode('windows');
      }
    };
    items.appendChild(el);
  });
  bindListDrag(items, {
    onReorder(visibleKeys){
      const full = orderedByKeys(appList, appOrder, a => a.path).map(a => a.path);
      for (const a of appList) if (!full.includes(a.path)) full.push(a.path);
      appOrder = mergeVisibleOrder(full, visibleKeys);
      const alive = new Set(appList.map(a => a.path));
      appOrder = appOrder.filter(p => alive.has(p));
      saveAppOrder();
    }
  });
}

function isMobileLayout(){
  return window.matchMedia('(max-width: 768px)').matches;
}
function openSidebar(){
  document.body.classList.remove('side-collapsed');
  syncMenuBtn();
}
function closeSidebar(){
  document.body.classList.add('side-collapsed');
  syncMenuBtn();
  // 목록 닫힌 뒤 원격 화면 다시 맞춤
  requestAnimationFrame(fitCanvas);
}
function toggleSidebar(){
  if (document.body.classList.contains('side-collapsed')) openSidebar();
  else closeSidebar();
}
function syncMenuBtn(){
  const btn = document.getElementById('menuBtn');
  if (!btn) return;
  const open = !document.body.classList.contains('side-collapsed');
  btn.setAttribute('aria-expanded', open ? 'true' : 'false');
  btn.textContent = open ? t('close') : t('menu');
}
function toggleBar(){
  document.body.classList.toggle('bar-collapsed');
  requestAnimationFrame(fitCanvas);
}

// 모바일: 목록 드로어 기본 닫힘 → 헤더(CPU 등) + 원격 화면 최대.
// 데스크톱: 사이드 항상 열림.
function syncLayoutMode(){
  if (isMobileLayout()) {
    closeSidebar();
  } else {
    document.body.classList.remove('side-collapsed');
    syncMenuBtn();
  }
  fitCanvas();
}
window.addEventListener('resize', () => {
  if (!isMobileLayout()) {
    document.body.classList.remove('side-collapsed');
    syncMenuBtn();
  }
  fitCanvas();
});
syncLayoutMode();

function isTypingTarget(el){
  if (!el) return false;
  const t = el.tagName;
  return t === 'INPUT' || t === 'TEXTAREA' || el.isContentEditable;
}
// 단축키 없음 — 타자 입력(원격 포함)과 충돌 방지. 사이드바/탭은 UI 버튼으로만.

function clientXY(e){
  if (e.touches && e.touches[0]) return { x: e.touches[0].clientX, y: e.touches[0].clientY };
  if (e.changedTouches && e.changedTouches[0]) return { x: e.changedTouches[0].clientX, y: e.changedTouches[0].clientY };
  return { x: e.clientX, y: e.clientY };
}
function norm(e){
  const r = cv.getBoundingClientRect();
  const { x, y } = clientXY(e);
  if (r.width < 1 || r.height < 1) return { x: 0.5, y: 0.5 };
  return { x: (x - r.left) / r.width, y: (y - r.top) / r.height };
}
function oob(p){ return p.x<0||p.y<0||p.x>1||p.y>1; }
function clamp(v){ return Math.max(0, Math.min(1, v)); }

let dragging = false, curBtn = 0, lastMove = 0, curClickCount = 1;
// 원격 지연 대비: 로컬에서 multi-click 판정 후 clickState(count)로 전달.
// 네트워크 RTT가 있어도 Mac은 clickState=2면 더블클릭으로 처리.
const MULTI_CLICK_MS = 700;
const MULTI_CLICK_DIST = 0.045;
// 이 거리 미만 이동은 "클릭" — move(드래그) 이벤트를 보내지 않음.
// (미세 지터가 leftMouseDragged로 나가면 클릭이 씹힘)
const DRAG_THRESH = 0.012;
let multiClick = { t: 0, x: 0, y: 0, count: 0, button: 0 };
let downOrigin = { x: 0, y: 0 };
let dragStarted = false;
let downSent = false;

function multiClickCount(p, button){
  const now = performance.now();
  const dist = Math.hypot(p.x - multiClick.x, p.y - multiClick.y);
  if (button === multiClick.button && now - multiClick.t < MULTI_CLICK_MS && dist < MULTI_CLICK_DIST) {
    multiClick.count = Math.min(3, multiClick.count + 1);
  } else {
    multiClick.count = 1;
  }
  multiClick.t = now;
  multiClick.x = p.x;
  multiClick.y = p.y;
  multiClick.button = button;
  return multiClick.count;
}

function sendMouseDown(p, button, count){
  downSent = true;
  send({ type:'down', x:p.x, y:p.y, button, count });
}

cv.addEventListener('pointerdown', e => {
  if (e.pointerType === 'touch') return; // 터치는 stage 핸들러
  if (e.button !== 0 && e.button !== 2) return;
  // preventDefault 하면 일부 브라우저에서 mousedown이 안 옴 → 즉시 전송.
  e.preventDefault();
  try { cv.focus({ preventScroll: true }); } catch (_) { try { cv.focus(); } catch (_) {} }
  const p = norm(e); if (oob(p)) return;
  curBtn = e.button;
  curClickCount = multiClickCount(p, e.button);
  downOrigin = { x: p.x, y: p.y };
  dragStarted = false;
  downSent = false;
  dragging = true;
  try { cv.setPointerCapture(e.pointerId); } catch (_) {}
  sendMouseDown(p, e.button, curClickCount);
});

// mousedown.detail 보강 (preventDefault 전에 이미 down 보낸 뒤라도 count 수정은 불가 →
// 다음 클릭부터 multiClick이 맞으면 충분. detail만 더 큰 경우 업 이벤트 count에 반영)
cv.addEventListener('mousedown', e => {
  if (e.button !== 0 && e.button !== 2) return;
  if (!dragging) return;
  const d = e.detail | 0;
  if (d >= 2 && d > curClickCount) {
    curClickCount = Math.min(3, d);
    multiClick.count = curClickCount;
    multiClick.t = performance.now();
  }
});

cv.addEventListener('pointermove', e => {
  if (!dragging || e.pointerType === 'touch') return;
  if (!downSent) return;
  const p = norm(e);
  const dist = Math.hypot(p.x - downOrigin.x, p.y - downOrigin.y);
  // 클릭 지터: 임계 전엔 원격에 move를 보내지 않음 (드래그로 오인 → 클릭 씹힘 방지)
  if (!dragStarted) {
    if (dist < DRAG_THRESH) return;
    dragStarted = true;
    // 진짜 드래그 시작 시에만 multi-click 시퀀스 리셋
    multiClick.count = 1;
    curClickCount = 1;
  }
  const now = performance.now();
  if (now - lastMove < 8) return;
  lastMove = now;
  send({ type:'move', x:clamp(p.x), y:clamp(p.y), button:curBtn, count:1 });
});

function endPointerDrag(e){
  if (!dragging || e.pointerType === 'touch') return;
  dragging = false;
  try { cv.releasePointerCapture(e.pointerId); } catch (_) {}
  if (!downSent) return;
  const p = norm(e);
  // up도 동일 clickState 유지 (AppKit 더블클릭 필수)
  const count = dragStarted ? 1 : curClickCount;
  send({ type:'up', x:clamp(p.x), y:clamp(p.y), button:curBtn, count });
  // 더블클릭 윈도우는 up 시각 기준으로 연장
  if (!dragStarted) {
    multiClick.t = performance.now();
    multiClick.x = clamp(p.x);
    multiClick.y = clamp(p.y);
  }
  downSent = false;
  dragStarted = false;
}
cv.addEventListener('pointerup', endPointerDrag);
cv.addEventListener('pointercancel', endPointerDrag);
cv.addEventListener('contextmenu', e => e.preventDefault());

let accDX = 0, accDY = 0, wheelTimer = null, wheelPt = {x:0.5,y:0.5};
cv.addEventListener('wheel', e => {
  // 데스크톱: 트랙패드 핀치(ctrl+wheel) → 로컬 줌, 일반 휠 → 원격 스크롤
  if (e.ctrlKey || e.metaKey) {
    e.preventDefault();
    const stage = document.getElementById('stage');
    const rect = stage.getBoundingClientRect();
    const factor = Math.exp(-e.deltaY * 0.01);
    setZoom(view.zoom * factor, e.clientX - rect.left, e.clientY - rect.top);
    return;
  }
  e.preventDefault();
  wheelPt = norm(e);
  accDX += e.deltaX; accDY += e.deltaY;
  if (!wheelTimer) wheelTimer = setTimeout(flushWheel, 20);
}, {passive:false});
function flushWheel(){
  wheelTimer = null;
  if (accDX || accDY) {
    send({type:'scroll', x:wheelPt.x, y:wheelPt.y, dx:accDX, dy:accDY});
    accDX = 0; accDY = 0;
  }
}

// ── 터치: 한 손가락 = 원격 조작, 두 손가락 = 화면 확대/이동 ──
const stageEl = document.getElementById('stage');
let touchMode = null; // 'remote' | 'view'
let pinchStartDist = 0, pinchStartZoom = 1;
let panStartX = 0, panStartY = 0, panOriginX = 0, panOriginY = 0;
let lastTapAt = 0;

function touchDist(a, b){
  const dx = a.clientX - b.clientX, dy = a.clientY - b.clientY;
  return Math.hypot(dx, dy);
}
function touchMid(a, b){
  return { x: (a.clientX + b.clientX) / 2, y: (a.clientY + b.clientY) / 2 };
}

stageEl.addEventListener('touchstart', e => {
  if (e.target !== cv && e.target !== stageEl) return;
  if (e.touches.length === 1) {
    touchMode = 'remote';
    e.preventDefault();
    cv.focus();
    const p = norm(e);
    if (oob(p)) return;
    // 더블탭 → 맞춤 리셋
    const now = performance.now();
    if (now - lastTapAt < 280) {
      resetViewAuto();
      lastTapAt = 0;
      touchMode = null;
      return;
    }
    lastTapAt = now;
    curBtn = 0; dragging = true;
    send({type:'down', x:p.x, y:p.y, button:0, count:1});
  } else if (e.touches.length >= 2) {
    // 원격 제스처 취소하고 뷰 조작
    if (dragging) {
      const p = norm(e);
      send({type:'up', x:clamp(p.x), y:clamp(p.y), button:0, count:1});
      dragging = false;
    }
    touchMode = 'view';
    e.preventDefault();
    view.mode = 'free';
    pinchStartDist = touchDist(e.touches[0], e.touches[1]);
    pinchStartZoom = view.zoom;
    panStartX = touchMid(e.touches[0], e.touches[1]).x;
    panStartY = touchMid(e.touches[0], e.touches[1]).y;
    panOriginX = view.panX;
    panOriginY = view.panY;
  }
}, {passive:false});

stageEl.addEventListener('touchmove', e => {
  if (touchMode === 'remote' && e.touches.length === 1 && dragging) {
    e.preventDefault();
    const now = performance.now(); if (now - lastMove < 16) return; lastMove = now;
    const p = norm(e);
    send({type:'move', x:clamp(p.x), y:clamp(p.y), button:0});
  } else if (touchMode === 'view' && e.touches.length >= 2) {
    e.preventDefault();
    const d = touchDist(e.touches[0], e.touches[1]);
    const mid = touchMid(e.touches[0], e.touches[1]);
    const stage = document.getElementById('stage');
    const rect = stage.getBoundingClientRect();
    if (pinchStartDist > 8) {
      const factor = d / pinchStartDist;
      view.zoom = Math.max(0.5, Math.min(5, pinchStartZoom * factor));
    }
    view.panX = panOriginX + (mid.x - panStartX);
    view.panY = panOriginY + (mid.y - panStartY);
    applyView();
    updateZoomHint();
  }
}, {passive:false});

stageEl.addEventListener('touchend', e => {
  if (touchMode === 'remote' && dragging && e.touches.length === 0) {
    e.preventDefault();
    dragging = false;
    const p = norm(e);
    send({type:'up', x:clamp(p.x), y:clamp(p.y), button:0, count:1});
    touchMode = null;
  } else if (e.touches.length === 0) {
    touchMode = null;
  } else if (e.touches.length === 1 && touchMode === 'view') {
    // 한 손가락만 남으면 뷰 종료
    touchMode = null;
  }
}, {passive:false});

// iOS 사파리 제스처 기본 동작 방지 (페이지 줌)
document.addEventListener('gesturestart', e => e.preventDefault());
document.addEventListener('gesturechange', e => e.preventDefault());
window.addEventListener('keydown', e => {
  if ((e.metaKey || e.ctrlKey) && ['=','-','+','0'].includes(e.key)) e.preventDefault();
});

// 입력칸(#txt, 검색)에서는 로컬 Cmd+C/V 유지.
// 캔버스: Cmd+V → 이 기기 클립보드 → 원격 / Cmd+C·X → 원격 복사 → 이 기기 클립보드.
const SPECIAL = ['Backspace','Enter','Tab','Escape','Delete','ArrowLeft','ArrowRight','ArrowUp','ArrowDown','Home','End','PageUp','PageDown','NumpadEnter'];

// Remote host is always macOS. On Windows/Linux, Ctrl is the primary shortcut key → map to Command.
function isAppleClient(){
  try {
    const p = navigator.platform || '';
    if (/Mac|iPhone|iPad|iPod/i.test(p)) return true;
    const ua = navigator.userAgent || '';
    if (/Macintosh|Mac OS X|iPhone|iPad|iPod/i.test(ua)) return true;
    if (navigator.userAgentData && navigator.userAgentData.platform) {
      const plat = String(navigator.userAgentData.platform);
      if (/macOS|iOS|Mac/i.test(plat)) return true;
      if (/Win|Linux|Android|Chrome OS/i.test(plat)) return false;
    }
  } catch (_) {}
  return false;
}
const mapCtrlToCmd = !isAppleClient();

/** Browser modifiers → Mac host modifiers (Ctrl→⌘ on non-Apple clients). */
function remoteModifiers(e){
  let meta = !!e.metaKey;
  let ctrl = !!e.ctrlKey;
  if (mapCtrlToCmd && ctrl && !e.metaKey) {
    meta = true;
    ctrl = false;
  }
  return { meta, ctrl, alt: !!e.altKey, shift: !!e.shiftKey };
}

async function pasteClipboardToRemote(){
  try {
    if (navigator.clipboard && navigator.clipboard.readText) {
      const clip = await navigator.clipboard.readText();
      if (clip) { send({type:'text', value:clip}); statusEl.textContent = t('paste')+' ('+clip.length+')'; return; }
    }
  } catch (_) { /* clipboard denied → remote Cmd+V */ }
  send({type:'key', code:'KeyV', meta:true, ctrl:false, alt:false, shift:false});
  statusEl.textContent = t('pasteRemote');
}

let lastRemoteClip = '';

function toggleClipAuto(){
  clipAuto = !clipAuto;
  localStorage.setItem('webdock.clipAuto', clipAuto ? '1' : '0');
  syncClipAutoBtn();
  send({type:'clipAuto', value: clipAuto});
  statusEl.textContent = clipAuto ? t('clipOn') : t('clipOff');
}
function syncClipAutoBtn(){
  const btn = document.getElementById('clipAutoBtn');
  if (!btn) return;
  btn.classList.toggle('on', clipAuto);
  btn.setAttribute('aria-pressed', clipAuto ? 'true' : 'false');
  btn.textContent = clipAuto ? t('clipAutoOn') : t('clipAutoOff');
  btn.title = clipAuto ? t('clipOn') : t('clipOff');
}

/** 수동: 원격 Mac 클립보드 현재 내용 요청 (유저 제스처 → write 권한 유리) */
function pullRemoteClipboard(){
  statusEl.textContent = t('clipPulling');
  // 이 클릭이 유저 제스처이므로, 응답 후 writeText 성공할 확률 ↑
  window._clipPullExpect = true;
  send({type:'clipboardGet'});
}

/** Write remote Mac pasteboard text into this browser's clipboard. */
async function applyRemoteClipboard(m){
  const t = (m && m.value != null) ? String(m.value) : '';
  const fromManual = !!window._clipPullExpect;
  window._clipPullExpect = false;
  if (!clipAuto && !m.force && !fromManual) return;

  if (!t) {
    statusEl.textContent = t('clipEmpty');
    return;
  }
  lastRemoteClip = t;

  try {
    if (navigator.clipboard && navigator.clipboard.writeText) {
      await navigator.clipboard.writeText(t);
      statusEl.textContent = t('clipOk')+' ('+t.length+')';
      return;
    }
  } catch (_) { /* fall through */ }

  try {
    const ta = document.createElement('textarea');
    ta.value = t;
    ta.setAttribute('readonly', '');
    ta.style.cssText = 'position:fixed;left:0;top:0;width:1px;height:1px;opacity:0';
    document.body.appendChild(ta);
    ta.focus();
    ta.select();
    ta.setSelectionRange(0, t.length);
    const ok = document.execCommand('copy');
    document.body.removeChild(ta);
    if (ok) {
      statusEl.textContent = t('clipOk')+' ('+t.length+')';
      return;
    }
  } catch (_) { /* fall through */ }

  const txt = document.getElementById('txt');
  if (txt) {
    txt.value = t;
    txt.focus();
    txt.select();
    statusEl.textContent = t('clipBlocked')+' ('+t.length+')';
  } else {
    statusEl.textContent = t('clipBlocked')+' ('+t.length+')';
  }
}

// ── Typing
// 한글(한): 브라우저 2벌식 조합 → 완성 유니코드 전송 (Mac IME 조합 깨짐 회피)
// 영문(A): 물리 키 코드 → Mac
// 단독 모음(ㅏㅓ…) 허용. Windows 로컬 한글은 꺼 두는 것을 권장.

function isRemoteKeyTarget(el){
  if (!el) return false;
  if (el === cv || el === stageEl) return true;
  if (typeof el.closest === 'function' && el.closest('#stage')) return true;
  return document.activeElement === cv;
}

function isPhysicalKeyCode(code){
  if (!code) return false;
  return code.startsWith('Key') || code.startsWith('Digit') || code.startsWith('Numpad') ||
    code === 'Space' || code === 'CapsLock' || SPECIAL.includes(code) ||
    ['Minus','Equal','BracketLeft','BracketRight','Backslash','Semicolon','Quote',
     'Comma','Period','Slash','Backquote'].includes(code);
}

function physicalShift(e){ return !!e.shiftKey; }

function sendPhysicalKey(e, mod){
  send({
    type: 'key',
    code: e.code,
    meta: !!(mod && mod.meta),
    ctrl: !!(mod && mod.ctrl),
    alt: !!(mod ? mod.alt : e.altKey),
    shift: physicalShift(e),
    ime: !!imeKorean
  });
}

// ── 2벌식 Hangul (client compose → unicode text)
const H_CHO = 'ㄱㄲㄴㄷㄸㄹㅁㅂㅃㅅㅆㅇㅈㅉㅊㅋㅌㅍㅎ';
const H_JUNG = 'ㅏㅐㅑㅒㅓㅔㅕㅖㅗㅘㅙㅚㅛㅜㅝㅞㅟㅠㅡㅢㅣ';
const H_JONG = 'ㄱㄲㄳㄴㄵㄶㄷㄹㄺㄻㄼㄽㄾㄿㅀㅁㅂㅄㅅㅆㅇㅈㅊㅋㅌㅍㅎ'; // T = index+1
const H_KEY = {
  KeyQ:'ㅂ', KeyW:'ㅈ', KeyE:'ㄷ', KeyR:'ㄱ', KeyT:'ㅅ', KeyY:'ㅛ', KeyU:'ㅕ', KeyI:'ㅑ', KeyO:'ㅐ', KeyP:'ㅔ',
  KeyA:'ㅁ', KeyS:'ㄴ', KeyD:'ㅇ', KeyF:'ㄹ', KeyG:'ㅎ', KeyH:'ㅗ', KeyJ:'ㅓ', KeyK:'ㅏ', KeyL:'ㅣ',
  KeyZ:'ㅋ', KeyX:'ㅌ', KeyC:'ㅊ', KeyV:'ㅍ', KeyB:'ㅠ', KeyN:'ㅜ', KeyM:'ㅡ'
};
const H_KEY_SHIFT = {
  KeyQ:'ㅃ', KeyW:'ㅉ', KeyE:'ㄸ', KeyR:'ㄲ', KeyT:'ㅆ', KeyO:'ㅒ', KeyP:'ㅖ'
};
const H_JUNG_COMB = {
  'ㅗㅏ':'ㅘ','ㅗㅐ':'ㅙ','ㅗㅣ':'ㅚ',
  'ㅜㅓ':'ㅝ','ㅜㅔ':'ㅞ','ㅜㅣ':'ㅟ',
  'ㅡㅣ':'ㅢ'
};
const H_JONG_COMB = {
  'ㄱㅅ':'ㄳ','ㄴㅈ':'ㄵ','ㄴㅎ':'ㄶ',
  'ㄹㄱ':'ㄺ','ㄹㅁ':'ㄻ','ㄹㅂ':'ㄼ','ㄹㅅ':'ㄽ','ㄹㅌ':'ㄾ','ㄹㅍ':'ㄿ','ㄹㅎ':'ㅀ',
  'ㅂㅅ':'ㅄ'
};
// reverse: complex → [first, second] for backspace / detach
const H_JONG_SPLIT = {};
Object.keys(H_JONG_COMB).forEach(k => { H_JONG_SPLIT[H_JONG_COMB[k]] = [k[0], k[1]]; });
const H_JUNG_SPLIT = {};
Object.keys(H_JUNG_COMB).forEach(k => { H_JUNG_SPLIT[H_JUNG_COMB[k]] = [k[0], k[1]]; });

function hIsVowel(j){ return H_JUNG.indexOf(j) >= 0; }
function hIsConsonant(j){ return H_CHO.indexOf(j) >= 0 || H_JONG.indexOf(j) >= 0; }
function hSyllable(L, V, T){
  // L,V indices; T is jong index 0..27 (0=none)
  return String.fromCharCode(0xAC00 + (L * 21 + V) * 28 + T);
}
function hChoIdx(j){ return H_CHO.indexOf(j); }
function hJungIdx(j){ return H_JUNG.indexOf(j); }
function hJongIdx(j){ // 1-based in syllable, 0 if none
  if (!j) return 0;
  const i = H_JONG.indexOf(j);
  return i < 0 ? 0 : i + 1;
}
function hJongChar(t){ return t > 0 ? H_JONG[t - 1] : ''; }

// composer: {L,V,T} indices (T 0=none); empty all -1 / 0
let hState = { L:-1, V:-1, T:0 };
let hComposeLen = 0; // remote chars still in "composing" region (replace target)

function hComposingStr(){
  if (hState.L < 0) return '';
  if (hState.V < 0) return H_CHO[hState.L];
  return hSyllable(hState.L, hState.V, hState.T);
}
function hReset(){ hState = { L:-1, V:-1, T:0 }; }
function hHas(){ return hState.L >= 0; }

/** Push composing region to remote: delete hComposeLen, write str, set len. */
function hSyncRemote(str){
  const rep = hComposeLen;
  const chars = str || '';
  hComposeLen = chars.length;
  if (rep === 0 && !chars) return;
  send({ type:'text', value: chars, replace: rep });
}

/** Make composing permanent on remote (stop replacing it). */
function hangulFlush(sendSpace){
  hComposeLen = 0;
  hReset();
  if (sendSpace) sendPhysicalKey({ code:'Space', shiftKey:false }, {meta:false, ctrl:false, alt:false});
}

/**
 * Feed one jamo. Returns nothing; updates remote via text/replace.
 * Bare vowels allowed (standalone ㅏ etc.).
 */
function hangulFeed(jamo){
  const isV = hIsVowel(jamo);
  const isC = hIsConsonant(jamo);

  if (!isV && !isC) return false;

  // Empty buffer
  if (hState.L < 0) {
    if (isV) {
      // bare vowel — write compatibility jamo, permanent (not composing)
      send({ type:'text', value: jamo, replace: 0 });
      return true;
    }
    hState.L = hChoIdx(jamo);
    if (hState.L < 0) {
      // jong-only char as initial?
      const asCho = hChoIdx(jamo);
      if (asCho < 0) { send({ type:'text', value: jamo, replace: 0 }); return true; }
      hState.L = asCho;
    }
    hSyncRemote(hComposingStr());
    return true;
  }

  // Have initial only
  if (hState.V < 0) {
    if (isC) {
      // commit previous consonant, start new
      hComposeLen = 0; // leave previous ㄱ on remote
      hState.L = hChoIdx(jamo);
      if (hState.L < 0) { hReset(); send({ type:'text', value: jamo, replace: 0 }); return true; }
      hSyncRemote(hComposingStr());
      return true;
    }
    // vowel attaches
    hState.V = hJungIdx(jamo);
    if (hState.V < 0) return true;
    hSyncRemote(hComposingStr());
    return true;
  }

  // Have L+V, maybe T
  if (hState.T === 0) {
    if (isV) {
      // try combine medial (ㅗ+ㅏ→ㅘ)
      const cur = H_JUNG[hState.V];
      const comb = H_JUNG_COMB[cur + jamo];
      if (comb) {
        hState.V = hJungIdx(comb);
        hSyncRemote(hComposingStr());
        return true;
      }
      // commit syllable, bare vowel next
      hComposeLen = 0;
      hReset();
      send({ type:'text', value: jamo, replace: 0 });
      return true;
    }
    // consonant → try as final
    const t = hJongIdx(jamo);
    if (t > 0) {
      // ㄸ ㅃ ㅉ cannot be finals
      hState.T = t;
      hSyncRemote(hComposingStr());
      return true;
    }
    // cannot be final — commit + new initial
    hComposeLen = 0;
    hState = { L: hChoIdx(jamo), V:-1, T:0 };
    if (hState.L < 0) { hReset(); return true; }
    hSyncRemote(hComposingStr());
    return true;
  }

  // Have L+V+T
  if (isV) {
    // detach final → new syllable initial + this vowel
    const finalCh = hJongChar(hState.T);
    const split = H_JONG_SPLIT[finalCh];
    if (split) {
      // complex final: leave first part, move second to next L
      hState.T = hJongIdx(split[0]);
      const kept = hComposingStr();
      const nextL = hChoIdx(split[1]);
      const nextV = hJungIdx(jamo);
      const nextSyl = (nextL >= 0 && nextV >= 0) ? hSyllable(nextL, nextV, 0) : (split[1] + jamo);
      // replace old syllable with kept+next
      const rep = hComposeLen;
      hComposeLen = nextSyl.length; // only trailing composing
      send({ type:'text', value: kept + nextSyl, replace: rep });
      // permanently keep `kept`: composeLen tracks only nextSyl
      // After send, remote has kept+nextSyl. We want composeLen = nextSyl.length.
      // But next replace must only delete nextSyl — good if we set composeLen = nextSyl.length.
      // However kept is before nextSyl; first composeLen chars from end? 
      // replace deletes LAST N characters from cursor — assumes cursor at end.
      // Backspace N times deletes nextSyl only if composeLen=nextSyl.length. Yes.
      hState = { L: nextL, V: nextV, T: 0 };
      return true;
    }
    // simple final → becomes next initial
    hState.T = 0;
    const kept = hComposingStr();
    const nextL = hChoIdx(finalCh);
    const nextV = hJungIdx(jamo);
    if (nextL < 0 || nextV < 0) {
      hComposeLen = 0;
      hReset();
      send({ type:'text', value: finalCh + jamo, replace: 0 });
      return true;
    }
    const nextSyl = hSyllable(nextL, nextV, 0);
    const rep = hComposeLen;
    send({ type:'text', value: kept + nextSyl, replace: rep });
    hComposeLen = nextSyl.length;
    hState = { L: nextL, V: nextV, T: 0 };
    return true;
  }

  // another consonant: try complex final, else commit + new L
  const curF = hJongChar(hState.T);
  const combF = H_JONG_COMB[curF + jamo];
  if (combF) {
    hState.T = hJongIdx(combF);
    hSyncRemote(hComposingStr());
    return true;
  }
  // commit current syllable, start new with jamo
  hComposeLen = 0;
  hState = { L: hChoIdx(jamo), V:-1, T:0 };
  if (hState.L < 0) { hReset(); send({ type:'text', value: jamo, replace: 0 }); return true; }
  hSyncRemote(hComposingStr());
  return true;
}

function hangulBackspace(){
  if (!hHas()) {
    if (hComposeLen > 0) {
      send({ type:'text', value:'', replace: hComposeLen });
      hComposeLen = 0;
      return true;
    }
    return false; // let physical Backspace through
  }
  if (hState.T > 0) {
    const ch = hJongChar(hState.T);
    const split = H_JONG_SPLIT[ch];
    if (split) {
      hState.T = hJongIdx(split[0]);
    } else {
      hState.T = 0;
    }
    hSyncRemote(hComposingStr());
    return true;
  }
  if (hState.V >= 0) {
    const cur = H_JUNG[hState.V];
    const split = H_JUNG_SPLIT[cur];
    if (split) {
      hState.V = hJungIdx(split[0]);
      hSyncRemote(hComposingStr());
      return true;
    }
    hState.V = -1;
    hSyncRemote(hComposingStr());
    return true;
  }
  // only L
  hReset();
  hSyncRemote('');
  return true;
}

function jamoFromCode(code, shift){
  if (shift && H_KEY_SHIFT[code]) return H_KEY_SHIFT[code];
  return H_KEY[code] || null;
}

// 로컬 브라우저 한글 조합 UI 억제 — 우리가 조합
cv.addEventListener('compositionstart', e => { e.preventDefault(); });
cv.addEventListener('compositionupdate', e => { e.preventDefault(); });
cv.addEventListener('compositionend', e => { e.preventDefault(); });

function handleRemoteKeydown(e){
  if (e.__webdockHandled) return;
  e.__webdockHandled = true;

  if (!isRemoteKeyTarget(e.target) && document.activeElement !== cv) return;
  if (isTypingTarget(e.target) || isTypingTarget(document.activeElement)) return;

  const physical = isPhysicalKeyCode(e.code);

  if (e.code === 'CapsLock') {
    e.preventDefault();
    e.stopPropagation();
    hangulFlush(false);
    sendPhysicalKey(e, {meta:false, ctrl:false, alt:false});
    return;
  }

  // Ctrl/Cmd+Space → 한/영 (client compose + Mac 동기)
  if ((e.ctrlKey || e.metaKey) && (e.code === 'Space' || e.key === ' ' || e.key === 'Spacebar')) {
    e.preventDefault();
    e.stopPropagation();
    if (typeof e.stopImmediatePropagation === 'function') e.stopImmediatePropagation();
    toggleIME();
    try { cv.focus(); } catch (_) {}
    return;
  }

  if (e.metaKey || e.ctrlKey) {
    const k = (e.key || '').toLowerCase();
    if (['=','-','+','0'].includes(e.key)) return;
    if (k === 'v' && !e.shiftKey && !e.altKey) {
      e.preventDefault();
      e.stopPropagation();
      hangulFlush(false);
      pasteClipboardToRemote();
      return;
    }
    if (physical) {
      e.preventDefault();
      e.stopPropagation();
      hangulFlush(false);
      sendPhysicalKey(e, remoteModifiers(e));
    }
    return;
  }

  // ── 한글 모드: 2벌식 클라이언트 조합
  if (imeKorean && !e.altKey) {
    if (e.code === 'Backspace') {
      e.preventDefault();
      e.stopPropagation();
      if (!hangulBackspace()) sendPhysicalKey(e, {meta:false, ctrl:false, alt:false});
      return;
    }
    if (e.code === 'Space') {
      e.preventDefault();
      e.stopPropagation();
      hangulFlush(true);
      return;
    }
    if (e.code === 'Enter' || e.code === 'NumpadEnter' || e.code === 'Tab' || e.code === 'Escape') {
      e.preventDefault();
      e.stopPropagation();
      hangulFlush(false);
      sendPhysicalKey(e, {meta:false, ctrl:false, alt:false});
      return;
    }
    if (SPECIAL.includes(e.code) && e.code !== 'Backspace') {
      e.preventDefault();
      e.stopPropagation();
      hangulFlush(false);
      sendPhysicalKey(e, {meta:false, ctrl:false, alt:false});
      return;
    }
    const jamo = jamoFromCode(e.code, e.shiftKey);
    if (jamo) {
      e.preventDefault();
      e.stopPropagation();
      hangulFeed(jamo);
      return;
    }
    // digits / punct: flush then physical
    if (physical || (e.key && e.key.length === 1)) {
      e.preventDefault();
      e.stopPropagation();
      hangulFlush(false);
      sendPhysicalKey(e, {meta:false, ctrl:false, alt:e.altKey});
      return;
    }
  }

  // ── 영문 모드: 물리 키
  if (physical || (e.key && e.key.length === 1) || e.altKey) {
    if (!e.code || e.code === 'Unidentified') {
      e.preventDefault();
      return;
    }
    e.preventDefault();
    e.stopPropagation();
    sendPhysicalKey(e, {meta:false, ctrl:false, alt:e.altKey});
  }
}

window.addEventListener('keydown', handleRemoteKeydown, true);

// paste / 하단 입력칸만 유니코드 텍스트 (클립보드 내용 그대로, IME 무관)
document.addEventListener('paste', e => {
  if (isTypingTarget(e.target)) return;
  if (activeId == null) return;
  const t = (e.clipboardData || window.clipboardData)?.getData('text/plain');
  if (!t) return;
  e.preventDefault();
  send({type:'text', value:t});
  statusEl.textContent = t('paste')+' ('+t.length+')';
  cv.focus();
});

document.getElementById('txt').addEventListener('keydown', e => { if(e.key==='Enter') sendText(); });
function sendText(){ const t = document.getElementById('txt'); if(t.value){ send({type:'text', value:t.value}); t.value=''; cv.focus(); } }
let streamFormat = 'jpeg'; // jpeg | png | h264
let streamPreset = 'balanced'; // fast | balanced | broadcast
let h264Decoder = null;
let h264Configured = false;
let h264WaitingKey = true;
let h264Desc = null;
let h264Codec = 'avc1.64001F';
let h264W = 0, h264H = 0;
let h264Ts = 0;
let h264PendingFrame = null;
let h264Raf = 0;
// Client stats → server adaptive bitrate (stable web stream)
let _statFrames = 0, _statDrops = 0, _statLast = performance.now();

function b64ToU8(b64){
  const bin = atob(b64);
  const u = new Uint8Array(bin.length);
  for (let i = 0; i < bin.length; i++) u[i] = bin.charCodeAt(i);
  return u;
}

function teardownH264(){
  try { if (h264Decoder) h264Decoder.close(); } catch (_) {}
  h264Decoder = null;
  h264Configured = false;
  h264WaitingKey = true;
  h264Desc = null;
  h264Ts = 0;
  if (h264PendingFrame) { try { h264PendingFrame.close(); } catch(_){} h264PendingFrame = null; }
  if (h264Raf) { cancelAnimationFrame(h264Raf); h264Raf = 0; }
}

function paintH264Frame(frame){
  const resized = (cv.width !== frame.displayWidth || cv.height !== frame.displayHeight);
  if (resized) {
    cv.width = frame.displayWidth;
    cv.height = frame.displayHeight;
  }
  ctx.imageSmoothingEnabled = true;
  ctx.imageSmoothingQuality = 'medium';
  ctx.drawImage(frame, 0, 0);
  cv.style.display = 'block';
  emptyEl.style.display = 'none';
  frames++;
  _statFrames++;
  if (resized || frames <= 2 || frames % 20 === 0) fitCanvas();
}

function setupH264Decoder(m){
  if (typeof VideoDecoder === 'undefined') {
    statusEl.textContent = t('h264Unsupported');
    return;
  }
  try {
    const nextCodec = m.codec || 'avc1.4D401F';
    const nextW = m.width | 0;
    const nextH = m.height | 0;
    const nextDesc = m.description ? b64ToU8(m.description) : null;
    // Same config → ignore (prevents freeze from re-configure every keyframe).
    const sameDesc = !!(h264Desc && nextDesc
      && h264Desc.byteLength === nextDesc.byteLength
      && h264Desc.every((b, i) => b === nextDesc[i]));
    if (h264Configured && h264Decoder && h264Decoder.state !== 'closed'
        && h264Codec === nextCodec && h264W === nextW && h264H === nextH && sameDesc) {
      return;
    }
    h264Codec = nextCodec;
    h264W = nextW;
    h264H = nextH;
    h264Desc = nextDesc;
    h264WaitingKey = true;
    h264Ts = 0;
    if (h264Decoder) {
      try { h264Decoder.close(); } catch (_) {}
    }
    h264Decoder = new VideoDecoder({
      output: (frame) => {
        // Keep newest decoded frame; paint on next display refresh (smooth + low lag).
        if (h264PendingFrame) {
          try { h264PendingFrame.close(); } catch (_) {}
          _statDrops++;
        }
        h264PendingFrame = frame;
        if (!h264Raf) {
          h264Raf = requestAnimationFrame(() => {
            h264Raf = 0;
            const f = h264PendingFrame;
            h264PendingFrame = null;
            if (!f) return;
            try { paintH264Frame(f); } finally { f.close(); }
          });
        }
      },
      error: (err) => {
        console.error('H264 decode', err);
        statusEl.textContent = t('h264DecodeErr');
        h264WaitingKey = true;
        if (activeId != null) send({type:'keyframe', id: activeId});
      }
    });
    const cfg = {
      codec: h264Codec,
      codedWidth: h264W || undefined,
      codedHeight: h264H || undefined,
      optimizeForLatency: true,
      hardwareAcceleration: 'prefer-hardware'
    };
    if (h264Desc && h264Desc.byteLength) cfg.description = h264Desc;
    h264Decoder.configure(cfg);
    h264Configured = true;
    statusEl.textContent = 'H.264 ' + h264W + '×' + h264H + ' · ' + t('h264Ok');
    if (activeId != null) send({type:'keyframe', id: activeId});
  } catch (err) {
    console.error(err);
    statusEl.textContent = t('h264Fail');
    teardownH264();
  }
}

function handleH264Sample(u8){
  if (!h264Configured || !h264Decoder || h264Decoder.state === 'closed') return;
  if (u8.byteLength < 14) return;
  const key = (u8[1] & 1) === 1;
  if (h264WaitingKey && !key) { _statDrops++; return; }
  if (key) h264WaitingKey = false;
  const view = new DataView(u8.buffer, u8.byteOffset, u8.byteLength);
  const len = view.getUint32(10);
  if (14 + len > u8.byteLength) return;
  const payload = u8.subarray(14, 14 + len);
  if (payload.byteLength < 5) return;
  try {
    // Prefer low latency: drop delta if decoder is backed up; never drop keys.
    const q = h264Decoder.decodeQueueSize;
    if (q > 1 && !key) { _statDrops++; return; }
    if (q > 3) { _statDrops++; return; }
    h264Ts += 33333;
    h264Decoder.decode(new EncodedVideoChunk({
      type: key ? 'key' : 'delta',
      timestamp: h264Ts,
      data: payload
    }));
    _statFrames++;
  } catch (err) {
    console.error('decode chunk', err);
    h264WaitingKey = true;
    _statDrops++;
    if (activeId != null) send({type:'keyframe', id: activeId});
  }
}

// Report playout health → server lowers bitrate under stress (stable web stream).
// Soften pressure after H.264 switch so we don't thrash the software encoder.
let _h264StatsGraceUntil = 0;
setInterval(() => {
  if (!ws || ws.readyState !== 1) return;
  if (streamFormat !== 'h264' && streamFormat !== 'jpeg') return;
  const now = performance.now();
  const dt = (now - _statLast) / 1000;
  if (dt < 0.8) return;
  const fps = _statFrames / dt;
  const drops = _statDrops;
  const queue = (h264Decoder && h264Decoder.state !== 'closed') ? h264Decoder.decodeQueueSize : 0;
  let pressure = 0;
  if (now < _h264StatsGraceUntil) {
    pressure = 0; // let first keyframes settle
  } else if (queue >= 4 || drops > 20 || fps < 8) pressure = 3;
  else if (queue >= 3 || drops > 10 || fps < 12) pressure = 2;
  else if (queue >= 2 || drops > 5 || fps < 16) pressure = 1;
  send({type:'stats', fps: Math.round(fps*10)/10, queue, drops, pressure});
  _statFrames = 0;
  _statDrops = 0;
  _statLast = now;
}, 1000);

function setQuality(v){
  if (streamFormat !== 'jpeg') return;
  send({type:'quality', value:parseFloat(v)});
  const el = document.getElementById('qVal');
  if (el) el.textContent = Math.round(parseFloat(v)*100) + '%';
}

function syncFormatButtons(f){
  const j = document.getElementById('fmtJpeg');
  const p = document.getElementById('fmtPng');
  const h = document.getElementById('fmtH264');
  if (j) j.classList.toggle('on', f === 'jpeg');
  if (p) p.classList.toggle('on', f === 'png');
  if (h) h.classList.toggle('on', f === 'h264');
  const q = document.getElementById('q');
  const lab = document.getElementById('qLabel');
  if (q) {
    q.disabled = (f !== 'jpeg');
    q.title = f === 'jpeg' ? t('jpegQ') : (f === 'png' ? t('pngLossless') : t('h264Auto'));
  }
  if (lab) lab.classList.toggle('dim', f !== 'jpeg');
}

function syncPresetButtons(name){
  const n = (name === 'fast' || name === 'broadcast') ? name : 'balanced';
  const preFast = document.getElementById('preFast');
  const preBal = document.getElementById('preBal');
  const preLive = document.getElementById('preLive');
  if (preFast) preFast.classList.toggle('on', n === 'fast');
  if (preBal) preBal.classList.toggle('on', n === 'balanced');
  if (preLive) preLive.classList.toggle('on', n === 'broadcast');
}

/**
 * H.264 needs WebCodecs VideoDecoder.
 * Do NOT hard-block on isSecureContext when VideoDecoder exists (some builds differ).
 * When VideoDecoder is missing, explain HTTPS/localhost for LAN.
 */
function canUseH264(){
  if (typeof VideoDecoder !== 'undefined') {
    return { ok: true };
  }
  const insecure = (typeof isSecureContext !== 'undefined' && !isSecureContext);
  return {
    ok: false,
    reason: insecure ? 'h264NeedsHttps' : 'noWebCodecs'
  };
}

function applyH264BlockedUi(reasonKey){
  const msg = t(reasonKey || 'noWebCodecs');
  statusEl.textContent = msg;
  // Keep streaming as JPEG so the picture does not freeze.
  streamFormat = 'jpeg';
  streamPreset = 'balanced';
  teardownH264();
  syncFormatButtons('jpeg');
  syncPresetButtons('balanced');
  const h = document.getElementById('fmtH264');
  const live = document.getElementById('preLive');
  if (h) h.title = msg;
  if (live) live.title = msg;
  // Tell server to stay/return on balanced JPEG if we never sent broadcast.
  send({ type: 'preset', value: 'balanced' });
}

function setFormat(fmt){
  let f = 'jpeg';
  if (fmt === 'png') f = 'png';
  else if (fmt === 'h264' || fmt === 'avc') f = 'h264';
  if (f === 'h264') {
    const chk = canUseH264();
    if (!chk.ok) {
      applyH264BlockedUi(chk.reason);
      return;
    }
  }
  streamFormat = f;
  if (f !== 'h264') teardownH264();
  else {
    h264WaitingKey = true;
    _h264StatsGraceUntil = performance.now() + 5000;
    _statFrames = 0;
    _statDrops = 0;
    // Selecting H264 should also highlight Live preset (same server mode family).
    streamPreset = 'broadcast';
    syncPresetButtons('broadcast');
  }
  // Leaving H.264: if Live was selected, fall UI preset to balanced for consistency.
  if (f !== 'h264' && streamPreset === 'broadcast') {
    streamPreset = 'balanced';
    syncPresetButtons('balanced');
  }
  send({ type: 'format', value: f });
  syncFormatButtons(f);
  statusEl.textContent =
    f === 'png' ? t('fmtPng') :
    f === 'h264' ? t('fmtH264') :
    t('fmtJpeg');
  if (f === 'h264' && activeId != null) send({ type: 'keyframe', id: activeId });
}

/** Fast / Balanced / Live (H.264) */
function setPreset(name){
  let n = (name === 'fast' || name === 'broadcast') ? name : 'balanced';
  if (n === 'broadcast') {
    const chk = canUseH264();
    if (!chk.ok) {
      applyH264BlockedUi(chk.reason);
      return;
    }
  }
  streamPreset = n;
  const map = {
    fast: { q: 0.62, fmt: 'jpeg', labelKey: 'presetFast' },
    balanced: { q: 0.92, fmt: 'jpeg', labelKey: 'presetBal' },
    broadcast: { q: 1.0, fmt: 'h264', labelKey: 'presetLive' }
  };
  const conf = map[n];
  streamFormat = conf.fmt;
  if (conf.fmt !== 'h264') teardownH264();
  else {
    h264WaitingKey = true;
    _h264StatsGraceUntil = performance.now() + 5000;
    _statFrames = 0;
    _statDrops = 0;
  }
  // Server first, then UI — so .on always reflects what we applied.
  send({ type: 'preset', value: n });
  syncPresetButtons(n);
  syncFormatButtons(conf.fmt);
  const q = document.getElementById('q');
  const qv = document.getElementById('qVal');
  if (q && conf.fmt === 'jpeg') { q.disabled = false; q.value = String(conf.q); }
  if (qv && conf.fmt === 'jpeg') qv.textContent = Math.round(conf.q * 100) + '%';
  statusEl.textContent = t(conf.labelKey);
  if (conf.fmt === 'h264' && activeId != null) send({ type: 'keyframe', id: activeId });
}

// Tooltip only when H.264 unavailable (do not break .on selection styling).
(function markH264Availability(){
  const chk = canUseH264();
  if (chk.ok) return;
  const msg = t(chk.reason);
  const h = document.getElementById('fmtH264');
  const live = document.getElementById('preLive');
  if (h) h.title = msg;
  if (live) live.title = msg;
})();

const stage = document.getElementById('stage'), grip = document.getElementById('grip');
function tickGrip(){
  if (activeId != null && cv.style.display !== 'none'){
    const cr = cv.getBoundingClientRect(), sr = stage.getBoundingClientRect();
    grip.style.display = 'block';
    grip.style.left = (cr.right - sr.left - 14) + 'px';
    grip.style.top  = (cr.bottom - sr.top - 14) + 'px';
  } else grip.style.display = 'none';
  requestAnimationFrame(tickGrip);
}
requestAnimationFrame(tickGrip);
let rz = false, rzX = 0, rzY = 0, rzW = 0, rzH = 0, ppx = 1, ppy = 1, rzLast = 0;
grip.addEventListener('mousedown', e => {
  e.preventDefault(); e.stopPropagation(); rz = true;
  rzX = e.clientX; rzY = e.clientY; rzW = selW; rzH = selH;
  const cr = cv.getBoundingClientRect(); ppx = (selW || cr.width) / cr.width; ppy = (selH || cr.height) / cr.height;
});
window.addEventListener('mousemove', e => {
  if (!rz) return; const now = performance.now(); if (now - rzLast < 50) return; rzLast = now;
  const nw = Math.max(240, Math.round(rzW + (e.clientX - rzX) * ppx));
  const nh = Math.max(160, Math.round(rzH + (e.clientY - rzY) * ppy));
  send({type:'resize', w:nw, h:nh});
});
window.addEventListener('mouseup', () => { rz = false; });

renderQuick();
updateBadges();
syncClipAutoBtn();
// Language UI (WebDock parity: one applyI18n + select.value = lang)
try {
  const sel = document.getElementById('langSelect');
  if (sel) sel.value = lang;
  applyI18n();
  if (typeof syncMenuBtn === 'function') syncMenuBtn();
  if (typeof syncClipAutoBtn === 'function') syncClipAutoBtn();
} catch (_) {}
connect();
