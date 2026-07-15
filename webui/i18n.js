// ── i18n (complete UI pack) ───────────────────────────
const I18N = {
  ko: {
    menu:'목록', menuTitle:'창·앱 목록', sideTitle:'창 · 앱', close:'닫기',
    quick:'퀵 런처', edit:'편집', done:'완료', clients:'접속 중',
    tabWin:'창', tabApp:'앱', emptyTitle:'창을 선택하세요',
    emptyBody:'상단 <b>목록</b>에서 창을 고르거나<br><b>앱</b> 탭에서 실행하세요',
    txtPh:'텍스트 입력 · Enter 전송', send:'전송', imeTitle:'한/영 전환',
    preFast:'빠름', preBal:'균형', preLive:'방송', quality:'화질',
    refresh:'목록 새로고침', fit:'맞춤', zoomIn:'확대', zoomOut:'축소',
    clipAutoOn:'📋 자동', clipAutoOff:'📋 수동', clipPull:'가져오기',
    connecting:'연결 중…', connected:'연결됨', disconnected:'연결 끊김 — 재시도 중…',
    connError:'연결 오류', authFail:'인증 실패 — 토큰을 확인하세요',
    busy:'다른 곳에서 입력 중입니다', themeLight:'라이트 모드로 전환', themeDark:'다크 모드로 전환',
    cancel:'취소', confirm:'확인', searchWin:'창 · 앱 이름 검색…', searchApp:'앱 검색…',
    loadingApps:'앱 목록을 불러오는 중…', noSearch:'검색 결과가 없습니다',
    pinQuick:'퀵 런처에 고정', unpin:'고정 해제', newWin:'새 창 열기',
    closeWin:'이 창만 닫기 (앱 전체 종료 아님)', refreshList:'목록 새로고침 중…',
    streaming:'스트리밍', closing:'창 닫는 중…', pinned:'고정됨', pinFail:'고정 실패',
    launching:'실행 중…', newInstance:'새 창 여는 중…',
    paste:'붙여넣기', pasteRemote:'원격 붙여넣기 (Mac 클립보드)',
    clipOn:'클립보드 자동 가져오기 ON', clipOff:'클립보드 자동 가져오기 OFF',
    clipPulling:'원격 클립보드 가져오는 중…', clipEmpty:'원격 클립보드 비어 있음',
    clipOk:'원격 복사 → 이 기기 클립보드', clipBlocked:'클립보드 쓰기 막힘',
    imeKo:'현재: 한글 — 클릭하면 영문(A)', imeEn:'현재: 영문(A) — 클릭하면 한글',
    fitDevice:'화면: 기기 해상도 맞춤', fitAll:'화면: 전체 맞춤',
    resFit:'해상도 맞춤', qhint:'앱 탭에서 + 로 자주 쓰는 앱을 고정하세요',
    qhintEdit:' · 삭제는 편집', lang:'언어', disk:'디스크',
    remove:'제거', unpinBody:'을(를) 퀵 런처에서 제거할까요?',
    closeWinTitle:'창 닫기', closeWinBody:' 창만 닫습니다. 같은 앱의 다른 창은 유지됩니다.',
    noWindows:'열린 창이 없습니다.<br>앱 탭에서 앱을 실행하세요.',
    noClients:'접속 중인 클라이언트 없음', noView:'화면 미선택',
    noTitle:'제목 없음', dragReorder:'드래그하여 순서 변경', live:'스트리밍 중',
    diskLine:'디스크',
    fmtJpeg:'포맷: JPEG', fmtPng:'포맷: PNG (무손실)', fmtH264:'포맷: H.264 방송 (하드웨어)',
    noWebCodecs:'WebCodecs 없음 — Chrome/Edge 최신 버전 필요',
    h264Unsupported:'이 브라우저는 WebCodecs(H.264) 미지원 — JPG로 전환하세요',
    h264NeedsHttps:'H.264는 HTTPS 또는 localhost에서만 가능 — LAN은 프록시 HTTPS 또는 JPG 사용',
    h264DecodeErr:'H.264 디코드 오류 — 키프레임 요청…',
    h264Ok:'H.264 안정 모드', h264Fail:'H.264 설정 실패 — JPG 사용',
    presetFast:'빠름 · JPEG 20fps', presetBal:'균형 · JPEG 30fps',
    presetLive:'방송 · H.264 30fps 저지연 (하드웨어)',
    jpegQ:'JPEG 화질', pngLossless:'PNG 무손실', h264Auto:'H.264 비트레이트는 서버 자동'
  },
  en: {
    menu:'Menu', menuTitle:'Windows & apps', sideTitle:'Windows · Apps', close:'Close',
    quick:'Quick launch', edit:'Edit', done:'Done', clients:'Connected',
    tabWin:'Windows', tabApp:'Apps', emptyTitle:'Select a window',
    emptyBody:'Pick a window from <b>Menu</b> or launch an app under <b>Apps</b>',
    txtPh:'Type text · Enter to send', send:'Send', imeTitle:'Hangul / Latin',
    preFast:'Fast', preBal:'Balanced', preLive:'Live', quality:'Quality',
    refresh:'Refresh list', fit:'Fit', zoomIn:'Zoom in', zoomOut:'Zoom out',
    clipAutoOn:'📋 Auto', clipAutoOff:'📋 Manual', clipPull:'Pull',
    connecting:'Connecting…', connected:'Connected', disconnected:'Disconnected — retrying…',
    connError:'Connection error', authFail:'Auth failed — check token',
    busy:'Another client is typing', themeLight:'Switch to light mode', themeDark:'Switch to dark mode',
    cancel:'Cancel', confirm:'OK', searchWin:'Search windows…', searchApp:'Search apps…',
    loadingApps:'Loading apps…', noSearch:'No results',
    pinQuick:'Pin to quick launch', unpin:'Unpin', newWin:'New window',
    closeWin:'Close this window only', refreshList:'Refreshing…',
    streaming:'Streaming', closing:'Closing window…', pinned:'Pinned', pinFail:'Pin failed',
    launching:'Launching…', newInstance:'Opening new window…',
    paste:'Paste', pasteRemote:'Remote paste (Mac clipboard)',
    clipOn:'Clipboard auto-pull ON', clipOff:'Clipboard auto-pull OFF',
    clipPulling:'Pulling remote clipboard…', clipEmpty:'Remote clipboard empty',
    clipOk:'Remote copy → this device', clipBlocked:'Clipboard write blocked',
    imeKo:'Hangul mode — click for Latin', imeEn:'Latin mode — click for Hangul',
    fitDevice:'View: fit device', fitAll:'View: fit all',
    resFit:'Resolution', qhint:'Pin apps from the Apps tab with +',
    qhintEdit:' · edit to remove', lang:'Language', disk:'Disk',
    remove:'Remove', unpinBody:'Remove from quick launch?',
    closeWinTitle:'Close window', closeWinBody:' will close. Other windows of the same app stay open.',
    noWindows:'No open windows.<br>Launch an app from the Apps tab.',
    noClients:'No connected clients', noView:'No screen selected',
    noTitle:'Untitled', dragReorder:'Drag to reorder', live:'Streaming',
    diskLine:'Disk',
    fmtJpeg:'Format: JPEG', fmtPng:'Format: PNG (lossless)', fmtH264:'Format: H.264 (hardware)',
    noWebCodecs:'No WebCodecs — need latest Chrome/Edge',
    h264Unsupported:'Browser has no WebCodecs H.264 — switch to JPG',
    h264NeedsHttps:'H.264 needs HTTPS or localhost — use reverse-proxy TLS or JPG on LAN',
    h264DecodeErr:'H.264 decode error — requesting keyframe…',
    h264Ok:'H.264 stable mode', h264Fail:'H.264 setup failed — use JPG',
    presetFast:'Fast · JPEG 20fps', presetBal:'Balanced · JPEG 30fps',
    presetLive:'Live · H.264 30fps low-latency',
    jpegQ:'JPEG quality', pngLossless:'PNG lossless', h264Auto:'H.264 bitrate is automatic'
  },
  ja: {
    menu:'一覧', menuTitle:'ウィンドウとアプリ', sideTitle:'ウィンドウ · アプリ', close:'閉じる',
    quick:'クイック起動', edit:'編集', done:'完了', clients:'接続中',
    tabWin:'ウィンドウ', tabApp:'アプリ', emptyTitle:'ウィンドウを選択',
    emptyBody:'<b>一覧</b>からウィンドウを選ぶか、<b>アプリ</b>から起動',
    txtPh:'テキスト · Enterで送信', send:'送信', imeTitle:'韓国語 / 英字',
    preFast:'高速', preBal:'バランス', preLive:'配信', quality:'画質',
    refresh:'一覧を更新', fit:'合わせる', zoomIn:'拡大', zoomOut:'縮小',
    clipAutoOn:'📋 自動', clipAutoOff:'📋 手動', clipPull:'取得',
    connecting:'接続中…', connected:'接続済み', disconnected:'切断 — 再接続中…',
    connError:'接続エラー', authFail:'認証失敗 — トークンを確認',
    busy:'他のクライアントが入力中', themeLight:'ライトモード', themeDark:'ダークモード',
    cancel:'キャンセル', confirm:'OK', searchWin:'ウィンドウ検索…', searchApp:'アプリ検索…',
    loadingApps:'読み込み中…', noSearch:'結果なし',
    pinQuick:'クイックに固定', unpin:'固定解除', newWin:'新しいウィンドウ',
    closeWin:'このウィンドウだけ閉じる', refreshList:'更新中…',
    streaming:'配信中', closing:'閉じています…', pinned:'固定済み', pinFail:'固定失敗',
    launching:'起動中…', newInstance:'新しいウィンドウ…',
    paste:'貼り付け', pasteRemote:'リモート貼り付け',
    clipOn:'クリップボード自動 ON', clipOff:'クリップボード自動 OFF',
    clipPulling:'取得中…', clipEmpty:'クリップボードが空です',
    clipOk:'リモート → この端末', clipBlocked:'書き込み不可',
    imeKo:'ハングル — クリックで英字', imeEn:'英字 — クリックでハングル',
    fitDevice:'表示: 端末に合わせる', fitAll:'表示: 全体',
    resFit:'解像度', qhint:'アプリタブで + を押して固定',
    qhintEdit:' · 編集で削除', lang:'言語', disk:'ディスク',
    remove:'削除', unpinBody:'をクイック起動から削除しますか？',
    closeWinTitle:'ウィンドウを閉じる', closeWinBody:' だけ閉じます。同じアプリの他の窓は残ります。',
    noWindows:'開いているウィンドウがありません。<br>アプリタブから起動してください。',
    noClients:'接続中のクライアントなし', noView:'画面未選択',
    noTitle:'無題', dragReorder:'ドラッグで並べ替え', live:'配信中',
    diskLine:'ディスク',
    fmtJpeg:'形式: JPEG', fmtPng:'形式: PNG（可逆）', fmtH264:'形式: H.264（ハードウェア）',
    noWebCodecs:'WebCodecsなし — 最新Chrome/Edgeが必要',
    h264Unsupported:'H.264非対応 — JPGに切替',
    h264NeedsHttps:'H.264はHTTPS/localhostのみ — LANはTLSプロキシかJPG',
    h264DecodeErr:'H.264デコードエラー — キーフレーム要求…',
    h264Ok:'H.264 安定モード', h264Fail:'H.264設定失敗 — JPG使用',
    presetFast:'高速 · JPEG 20fps', presetBal:'バランス · JPEG 30fps',
    presetLive:'配信 · H.264 30fps 低遅延',
    jpegQ:'JPEG画質', pngLossless:'PNG可逆', h264Auto:'H.264ビットレートは自動'
  },
  zh: {
    menu:'列表', menuTitle:'窗口与应用', sideTitle:'窗口 · 应用', close:'关闭',
    quick:'快捷启动', edit:'编辑', done:'完成', clients:'已连接',
    tabWin:'窗口', tabApp:'应用', emptyTitle:'请选择窗口',
    emptyBody:'从顶部<b>列表</b>选择窗口，或在<b>应用</b>中启动',
    txtPh:'输入文字 · Enter 发送', send:'发送', imeTitle:'韩文 / 英文',
    preFast:'流畅', preBal:'均衡', preLive:'直播', quality:'画质',
    refresh:'刷新列表', fit:'适应', zoomIn:'放大', zoomOut:'缩小',
    clipAutoOn:'📋 自动', clipAutoOff:'📋 手动', clipPull:'拉取',
    connecting:'连接中…', connected:'已连接', disconnected:'已断开 — 重试中…',
    connError:'连接错误', authFail:'认证失败 — 请检查令牌',
    busy:'其他客户端正在输入', themeLight:'切换到浅色', themeDark:'切换到深色',
    cancel:'取消', confirm:'确定', searchWin:'搜索窗口…', searchApp:'搜索应用…',
    loadingApps:'正在加载应用…', noSearch:'无结果',
    pinQuick:'固定到快捷栏', unpin:'取消固定', newWin:'新窗口',
    closeWin:'仅关闭此窗口', refreshList:'刷新中…',
    streaming:'串流中', closing:'正在关闭…', pinned:'已固定', pinFail:'固定失败',
    launching:'启动中…', newInstance:'正在打开新窗口…',
    paste:'粘贴', pasteRemote:'远程粘贴（Mac 剪贴板）',
    clipOn:'剪贴板自动拉取 开', clipOff:'剪贴板自动拉取 关',
    clipPulling:'正在拉取…', clipEmpty:'远程剪贴板为空',
    clipOk:'远程复制 → 本机', clipBlocked:'无法写入剪贴板',
    imeKo:'韩文 — 点击切换英文', imeEn:'英文 — 点击切换韩文',
    fitDevice:'视图：适配设备', fitAll:'视图：全部适配',
    resFit:'分辨率', qhint:'在应用页用 + 固定常用应用',
    qhintEdit:' · 编辑可删除', lang:'语言', disk:'磁盘',
    remove:'移除', unpinBody:'要从快捷启动移除吗？',
    closeWinTitle:'关闭窗口', closeWinBody:' 将关闭。同应用的其他窗口保留。',
    noWindows:'没有打开的窗口。<br>请从应用页启动。',
    noClients:'无已连接客户端', noView:'未选择画面',
    noTitle:'无标题', dragReorder:'拖动排序', live:'串流中',
    diskLine:'磁盘',
    fmtJpeg:'格式: JPEG', fmtPng:'格式: PNG（无损）', fmtH264:'格式: H.264（硬件）',
    noWebCodecs:'无 WebCodecs — 请用最新 Chrome/Edge',
    h264Unsupported:'浏览器不支持 H.264 — 请改用 JPG',
    h264NeedsHttps:'H.264 仅 HTTPS/localhost — 局域网请用 TLS 代理或 JPG',
    h264DecodeErr:'H.264 解码错误 — 请求关键帧…',
    h264Ok:'H.264 稳定模式', h264Fail:'H.264 设置失败 — 使用 JPG',
    presetFast:'流畅 · JPEG 20fps', presetBal:'均衡 · JPEG 30fps',
    presetLive:'直播 · H.264 30fps 低延迟',
    jpegQ:'JPEG 画质', pngLossless:'PNG 无损', h264Auto:'H.264 码率由服务器自动'
  },
  de: {
    menu:'Liste', menuTitle:'Fenster & Apps', sideTitle:'Fenster · Apps', close:'Schließen',
    quick:'Schnellstart', edit:'Bearbeiten', done:'Fertig', clients:'Verbunden',
    tabWin:'Fenster', tabApp:'Apps', emptyTitle:'Fenster wählen',
    emptyBody:'Fenster über <b>Liste</b> wählen oder App unter <b>Apps</b> starten',
    txtPh:'Text · Enter senden', send:'Senden', imeTitle:'Hangul / Latein',
    preFast:'Schnell', preBal:'Ausgewogen', preLive:'Live', quality:'Qualität',
    refresh:'Liste aktualisieren', fit:'Anpassen', zoomIn:'Vergrößern', zoomOut:'Verkleinern',
    clipAutoOn:'📋 Auto', clipAutoOff:'📋 Manuell', clipPull:'Abrufen',
    connecting:'Verbinden…', connected:'Verbunden', disconnected:'Getrennt — erneut…',
    connError:'Verbindungsfehler', authFail:'Auth fehlgeschlagen — Token prüfen',
    busy:'Anderer Client tippt', themeLight:'Hellmodus', themeDark:'Dunkelmodus',
    cancel:'Abbrechen', confirm:'OK', searchWin:'Fenster suchen…', searchApp:'Apps suchen…',
    loadingApps:'Apps laden…', noSearch:'Keine Treffer',
    pinQuick:'Anheften', unpin:'Lösen', newWin:'Neues Fenster',
    closeWin:'Nur dieses Fenster schließen', refreshList:'Aktualisiere…',
    streaming:'Streaming', closing:'Schließe…', pinned:'Angeheftet', pinFail:'Anheften fehlgeschlagen',
    launching:'Startet…', newInstance:'Neues Fenster…',
    paste:'Einfügen', pasteRemote:'Remote-Einfügen',
    clipOn:'Zwischenablage Auto AN', clipOff:'Zwischenablage Auto AUS',
    clipPulling:'Lade Zwischenablage…', clipEmpty:'Remote-Zwischenablage leer',
    clipOk:'Remote → dieses Gerät', clipBlocked:'Schreiben blockiert',
    imeKo:'Hangul — Klick für Latein', imeEn:'Latein — Klick für Hangul',
    fitDevice:'Ansicht: Gerät', fitAll:'Ansicht: alles',
    resFit:'Auflösung', qhint:'Apps mit + anheften',
    qhintEdit:' · Bearbeiten zum Entfernen', lang:'Sprache', disk:'Disk',
    remove:'Entfernen', unpinBody:' vom Schnellstart entfernen?',
    closeWinTitle:'Fenster schließen', closeWinBody:' wird geschlossen. Andere Fenster der App bleiben.',
    noWindows:'Keine offenen Fenster.<br>App im Apps-Tab starten.',
    noClients:'Keine verbundenen Clients', noView:'Kein Bildschirm',
    noTitle:'Ohne Titel', dragReorder:'Ziehen zum Sortieren', live:'Streaming',
    diskLine:'Disk',
    fmtJpeg:'Format: JPEG', fmtPng:'Format: PNG (verlustfrei)', fmtH264:'Format: H.264 (Hardware)',
    noWebCodecs:'Kein WebCodecs — Chrome/Edge aktuell nötig',
    h264Unsupported:'Kein H.264 WebCodecs — JPG verwenden',
    h264NeedsHttps:'H.264 braucht HTTPS/localhost — LAN: TLS-Proxy oder JPG',
    h264DecodeErr:'H.264-Dekodierfehler — Keyframe…',
    h264Ok:'H.264 stabil', h264Fail:'H.264-Setup fehlgeschlagen — JPG',
    presetFast:'Schnell · JPEG 20fps', presetBal:'Ausgewogen · JPEG 30fps',
    presetLive:'Live · H.264 30fps niedrige Latenz',
    jpegQ:'JPEG-Qualität', pngLossless:'PNG verlustfrei', h264Auto:'H.264-Bitrate automatisch'
  },
  fr: {
    menu:'Liste', menuTitle:'Fenêtres et apps', sideTitle:'Fenêtres · Apps', close:'Fermer',
    quick:'Lancement rapide', edit:'Modifier', done:'OK', clients:'Connectés',
    tabWin:'Fenêtres', tabApp:'Apps', emptyTitle:'Choisir une fenêtre',
    emptyBody:'Choisissez via <b>Liste</b> ou lancez une app sous <b>Apps</b>',
    txtPh:'Texte · Entrée pour envoyer', send:'Envoyer', imeTitle:'Hangul / Latin',
    preFast:'Rapide', preBal:'Équilibré', preLive:'Live', quality:'Qualité',
    refresh:'Actualiser', fit:'Ajuster', zoomIn:'Zoom +', zoomOut:'Zoom −',
    clipAutoOn:'📋 Auto', clipAutoOff:'📋 Manuel', clipPull:'Récupérer',
    connecting:'Connexion…', connected:'Connecté', disconnected:'Déconnecté — nouvel essai…',
    connError:'Erreur de connexion', authFail:'Auth échouée — vérifier le jeton',
    busy:'Un autre client saisit', themeLight:'Mode clair', themeDark:'Mode sombre',
    cancel:'Annuler', confirm:'OK', searchWin:'Rechercher…', searchApp:'Rechercher apps…',
    loadingApps:'Chargement…', noSearch:'Aucun résultat',
    pinQuick:'Épingler', unpin:'Retirer', newWin:'Nouvelle fenêtre',
    closeWin:'Fermer cette fenêtre seule', refreshList:'Actualisation…',
    streaming:'Diffusion', closing:'Fermeture…', pinned:'Épinglé', pinFail:'Échec',
    launching:'Lancement…', newInstance:'Ouverture…',
    paste:'Coller', pasteRemote:'Coller distant',
    clipOn:'Presse-papiers auto ON', clipOff:'Presse-papiers auto OFF',
    clipPulling:'Récupération…', clipEmpty:'Presse-papiers distant vide',
    clipOk:'Distant → cet appareil', clipBlocked:'Écriture bloquée',
    imeKo:'Hangul — clic pour latin', imeEn:'Latin — clic pour hangul',
    fitDevice:'Vue : appareil', fitAll:'Vue : tout',
    resFit:'Résolution', qhint:'Épingler les apps avec +',
    qhintEdit:' · modifier pour retirer', lang:'Langue', disk:'Disque',
    remove:'Retirer', unpinBody:'Retirer du lancement rapide ?',
    closeWinTitle:'Fermer la fenêtre', closeWinBody:' sera fermée. Les autres fenêtres de l’app restent.',
    noWindows:'Aucune fenêtre ouverte.<br>Lancez une app dans l’onglet Apps.',
    noClients:'Aucun client connecté', noView:'Aucun écran',
    noTitle:'Sans titre', dragReorder:'Glisser pour réordonner', live:'En direct',
    diskLine:'Disque',
    fmtJpeg:'Format : JPEG', fmtPng:'Format : PNG (sans perte)', fmtH264:'Format : H.264 (matériel)',
    noWebCodecs:'Pas de WebCodecs — Chrome/Edge récent requis',
    h264Unsupported:'H.264 non supporté — passer en JPG',
    h264NeedsHttps:'H.264 exige HTTPS/localhost — LAN: proxy TLS ou JPG',
    h264DecodeErr:'Erreur décodage H.264 — image clé…',
    h264Ok:'H.264 mode stable', h264Fail:'Échec H.264 — utiliser JPG',
    presetFast:'Rapide · JPEG 20fps', presetBal:'Équilibré · JPEG 30fps',
    presetLive:'Live · H.264 30fps faible latence',
    jpegQ:'Qualité JPEG', pngLossless:'PNG sans perte', h264Auto:'Débit H.264 automatique'
  }
};

const LANG_KEY = 'webrust.lang';
const LANG_KEY_LEGACY = 'webdock.lang';

function detectLang(){
  try {
    const saved = localStorage.getItem(LANG_KEY) || localStorage.getItem(LANG_KEY_LEGACY);
    if (saved && I18N[saved]) return saved;
  } catch (_) {}
  const n = (navigator.language || 'en').toLowerCase();
  if (n.startsWith('ko')) return 'ko';
  if (n.startsWith('ja')) return 'ja';
  if (n.startsWith('zh')) return 'zh';
  if (n.startsWith('de')) return 'de';
  if (n.startsWith('fr')) return 'fr';
  return 'en';
}

/** Active UI language — single source of truth (WebDock-style). */
var lang = detectLang();

function t(key){
  const pack = I18N[lang] || I18N.en;
  return pack[key] || I18N.en[key] || key;
}

function setLang(code){
  // WebDock: invalid → en.
  if (!I18N[code]) code = 'en';
  lang = code;
  try {
    localStorage.setItem(LANG_KEY, code);
    localStorage.setItem(LANG_KEY_LEGACY, code);
  } catch (_) {}
  document.documentElement.lang = code === 'zh' ? 'zh-CN' : code;
  applyI18n();
  try { renderQuick(); } catch(_){}
  try { if (typeof render === 'function') render(); } catch(_){}
  try {
    if (typeof mode !== 'undefined' && mode === 'apps' && typeof renderAppItems === 'function') {
      renderAppItems();
    }
  } catch(_){}
  try { if (typeof renderClients === 'function') renderClients(window._lastClients || []); } catch(_){}
  try { if (typeof syncClipAutoBtn === 'function') syncClipAutoBtn(); } catch(_){}
  try {
    if (typeof applyIMEState === 'function') {
      applyIMEState(typeof imeKorean !== 'undefined' ? !!imeKorean : false);
    }
  } catch(_){}
  try { if (typeof syncMenuBtn === 'function') syncMenuBtn(); } catch(_){}
  // Only fix select if drifted — do not reassign during onchange when already correct.
  const sel = document.getElementById('langSelect');
  if (sel && sel.value !== code) sel.value = code;
}

function applyI18n(){
  document.querySelectorAll('[data-i18n]').forEach(el => {
    const k = el.getAttribute('data-i18n');
    if (!k) return;
    if (el.tagName === 'INPUT' || el.tagName === 'TEXTAREA' || el.tagName === 'SELECT') return;
    if (el.children && el.children.length && el.querySelector('button, input, select, textarea')) return;
    const html = el.getAttribute('data-i18n-html') === '1';
    if (html) el.innerHTML = t(k);
    else el.textContent = t(k);
  });
  document.querySelectorAll('[data-i18n-title]').forEach(el => {
    const k = el.getAttribute('data-i18n-title');
    if (k) el.title = t(k);
  });
  document.querySelectorAll('[data-i18n-placeholder]').forEach(el => {
    const k = el.getAttribute('data-i18n-placeholder');
    if (k) el.placeholder = t(k);
  });
  const cancel = document.getElementById('modalCancel');
  if (cancel) cancel.textContent = t('cancel');
  const ok = document.getElementById('modalOk');
  if (ok && (typeof modalResolve === 'undefined' || !modalResolve)) {
    ok.textContent = t('confirm');
  }
  const qeb = document.getElementById('quickEditBtn');
  if (qeb) qeb.textContent = (typeof quickEdit !== 'undefined' && quickEdit) ? t('done') : t('edit');
  const themeBtn = document.getElementById('themeBtn');
  if (themeBtn) {
    const light = document.documentElement.classList.contains('light');
    themeBtn.title = light ? t('themeDark') : t('themeLight');
  }
}
