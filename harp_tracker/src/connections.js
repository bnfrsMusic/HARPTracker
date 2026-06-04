const tauriCore = window.__TAURI__?.core ?? window.parent.__TAURI__?.core;
const tauriEvent = window.__TAURI__?.event ?? window.parent.__TAURI__?.event;

if (!tauriCore || !tauriEvent) {
    console.error('Tauri API not available — Connections must run inside the HARP Tracker app.');
}

const invoke = tauriCore.invoke.bind(tauriCore);
const listen = tauriEvent.listen.bind(tauriEvent);

const btnGs = document.getElementById('btn_gs');
const btnClient = document.getElementById('btn_client');
const choosePanel = document.getElementById('choose_panel');

const gsPanel = document.getElementById('gs_panel');
const clientPanel = document.getElementById('c_panel');

const gsIdDisplay = document.getElementById('gs_id_display');
const gsSignalingUrls = document.getElementById('gs_signaling_urls');
const signalingHostInput = document.getElementById('signaling_host_input');
const clientIdDisplay = document.getElementById('c_id_display');

const gsStatus = document.getElementById('gs_status');
const gsPendingStatus = document.getElementById('gs_pending_status');
const gsConnectedHint = document.getElementById('gs_connected_hint');
const clientStatus = document.getElementById('c_status');

const gsIdInput = document.getElementById('gs_id_input');
const clientNameInput = document.getElementById('client_name_input');

const pendingList = document.getElementById('pending_list');
const connectedClientList = document.getElementById('client_list');
const clientGsConnection = document.getElementById('c_gs_connection');

const clientConnectBtn = document.getElementById('client_accept_id');
const disconnectGsBtn = document.getElementById('btn_disconnect_gs');
const disconnectClientBtn = document.getElementById('btn_disconnect_c');

let currentRole = null;
let gsRunning = false;
let clientRunning = false;
let listenersReady = false;
let pendingPollTimer = null;

const pendingEntries = new Map();
const connectedEntries = new Map();

function notifyParent(eventName, payload) {
    if (window.parent && window.parent !== window) {
        window.parent.postMessage({ type: 'harp-connect-event', event: eventName, payload }, '*');
    }
}

function showPanel(panel) {
    choosePanel.hidden = true;
    gsPanel.hidden = true;
    clientPanel.hidden = true;
    if (panel) panel.hidden = false;
}

function showChoosePanel() {
    showPanel(choosePanel);
}

function setText(el, text) {
    if (el) el.textContent = text ?? '';
}

function setGsPendingStatus() {
    const n = pendingEntries.size;
    setText(
        gsPendingStatus,
        n === 0
            ? 'Offers are accepted automatically when a client connects.'
            : `${n} client${n === 1 ? '' : 's'} completing handshake…`
    );
}

function setGsConnectedStatus() {
    const n = connectedEntries.size;
    if (n === 0) {
        setText(gsConnectedHint, 'No clients connected yet.');
        setText(gsStatus, 'Online — waiting for clients');
    } else {
        setText(
            gsConnectedHint,
            `${n} client${n === 1 ? '' : 's'} receiving live flight data.`
        );
        setText(gsStatus, `Online — ${n} client${n === 1 ? '' : 's'} connected`);
    }
}

function makePeerCard({ peerId, name, clientId, statusLine, indicatorClass = 'pending', onRemove }) {
    const card = document.createElement('div');
    card.className = 'connection-entry remote-peer-card';
    card.dataset.peerId = peerId;

    const indicator = document.createElement('div');
    indicator.className = `conn-indicator ${indicatorClass}`;

    const body = document.createElement('div');
    body.className = 'remote-peer-body';

    if (name) {
        const nameRow = document.createElement('div');
        nameRow.className = 'remote-peer-row';
        const nameKey = document.createElement('span');
        nameKey.className = 'remote-peer-key';
        nameKey.textContent = 'Name';
        const nameVal = document.createElement('span');
        nameVal.className = 'remote-peer-val';
        nameVal.textContent = name;
        nameRow.appendChild(nameKey);
        nameRow.appendChild(nameVal);
        body.appendChild(nameRow);
    }

    const idRow = document.createElement('div');
    idRow.className = 'remote-peer-row';
    const idKey = document.createElement('span');
    idKey.className = 'remote-peer-key';
    idKey.textContent = 'Client ID';
    const idVal = document.createElement('code');
    idVal.className = 'remote-peer-id';
    idVal.textContent = clientId || peerId;
    idRow.appendChild(idKey);
    idRow.appendChild(idVal);
    body.appendChild(idRow);

    if (statusLine) {
        const statusRow = document.createElement('div');
        statusRow.className = 'remote-peer-row remote-peer-status-line';
        statusRow.dataset.statusLine = '1';
        const statusKey = document.createElement('span');
        statusKey.className = 'remote-peer-key';
        statusKey.textContent = 'Status';
        const statusVal = document.createElement('span');
        statusVal.className = 'remote-peer-val';
        statusVal.textContent = statusLine;
        statusRow.appendChild(statusKey);
        statusRow.appendChild(statusVal);
        body.appendChild(statusRow);
    }

    card.appendChild(indicator);
    card.appendChild(body);

    const actions = document.createElement('div');
    actions.className = 'remote-peer-actions';

    const copyBtn = document.createElement('button');
    copyBtn.type = 'button';
    copyBtn.className = 'remote-btn subtle';
    copyBtn.textContent = 'Copy ID';
    copyBtn.addEventListener('click', () => copyText(clientId || peerId));
    actions.appendChild(copyBtn);

    if (onRemove) {
        const removeBtn = document.createElement('button');
        removeBtn.type = 'button';
        removeBtn.className = 'remote-btn subtle danger-text';
        removeBtn.textContent = 'Remove';
        removeBtn.addEventListener('click', onRemove);
        actions.appendChild(removeBtn);
    }

    if (actions.childElementCount > 0) {
        card.appendChild(actions);
    }

    return card;
}

function updatePeerCardStatus(peerId, statusLine, indicatorClass) {
    const card = connectedEntries.get(peerId) || pendingEntries.get(peerId);
    if (!card) return;
    const line = card.querySelector('[data-status-line="1"] .remote-peer-val');
    if (line) line.textContent = statusLine;
    const dot = card.querySelector('.conn-indicator');
    if (dot && indicatorClass) {
        dot.className = `conn-indicator ${indicatorClass}`;
    }
}

async function copyText(text) {
    const value = (text || '').trim();
    if (!value || value === '—') return;
    try {
        await navigator.clipboard.writeText(value);
    } catch (_) {
        /* ignore */
    }
}

function setupCopyButtons() {
    document.querySelectorAll('.remote-copy').forEach((btn) => {
        btn.addEventListener('click', () => {
            const id = btn.dataset.copyTarget;
            const el = id ? document.getElementById(id) : null;
            if (el) copyText(el.textContent);
        });
    });
}

function removeEntry(map, peerId) {
    const entry = map.get(peerId);
    if (entry) {
        entry.remove();
        map.delete(peerId);
    }
}

function showPendingClient(id) {
    if (currentRole !== 'gs' || !id || pendingEntries.has(id) || connectedEntries.has(id)) {
        return;
    }

    const entry = makePeerCard({
        peerId: id,
        clientId: id,
        statusLine: 'Handshake in progress…',
        indicatorClass: 'pending',
    });

    pendingList.appendChild(entry);
    pendingEntries.set(id, entry);
    setGsPendingStatus();
    setText(gsStatus, `Client connecting — ID: ${id}`);
}

function showNewClient({ id, name }) {
    if (currentRole !== 'gs' || !id) return;
    removeEntry(pendingEntries, id);
    if (connectedEntries.has(id)) return;

    const displayName = name && name !== id ? name : null;
    const entry = makePeerCard({
        peerId: id,
        name: displayName,
        clientId: id,
        statusLine: 'Connected — syncing flight data',
        indicatorClass: 'ok',
        onRemove: async () => {
            try {
                await invoke('gs_remove_client', { nodeId: id });
            } catch (err) {
                console.error('Remove client failed:', err);
                setText(gsStatus, `Failed to remove ${id}`);
            }
        },
    });

    connectedClientList.appendChild(entry);
    connectedEntries.set(id, entry);
    setGsPendingStatus();
    setGsConnectedStatus();
}

function showLinkedGs({ id, name }) {
    clientGsConnection.replaceChildren();
    const displayName = name && name !== id ? name : 'Ground Station';
    const entry = makePeerCard({
        peerId: id,
        name: displayName,
        clientId: id,
        statusLine: 'Connected — receiving flight data',
        indicatorClass: 'ok',
    });
    clientGsConnection.appendChild(entry);
}

function handleConnectEvent(eventName, payload) {
    switch (eventName) {
        case 'pending-client':
            showPendingClient(payload.id);
            break;
        case 'new-client':
            showNewClient(payload);
            break;
        case 'client-removed':
            if (currentRole === 'gs') {
                removeEntry(connectedEntries, payload.id);
                removeEntry(pendingEntries, payload.id);
                setGsPendingStatus();
                setGsConnectedStatus();
            }
            break;
        case 'new-gs':
            if (currentRole === 'client') {
                showLinkedGs({
                    id: payload.id,
                    name: payload.name || payload.role,
                });
                setText(clientStatus, `Linked to Ground Station: ${payload.id}`);
                notifyParent('client-mode', { connected: true });
            }
            break;
        case 'client-mode':
            if (currentRole === 'client') {
                notifyParent('client-mode', payload);
                if (payload.connected) {
                    setText(clientStatus, 'Connected — receiving flight data from Ground Station');
                }
            }
            break;
        case 'gs-sync-full':
        case 'gs-sync-position':
        case 'gs-sync-prediction':
        case 'gs-disconnected':
            notifyParent(eventName, payload);
            break;
        case 'gs-online':
            if (currentRole === 'gs' && payload.id) {
                setText(gsIdDisplay, payload.id);
                setText(gsStatus, `Online — Ground Station ID: ${payload.id}`);
            }
            break;
        case 'client-error': {
            const message = payload?.message ?? 'Connection error';
            if (message.includes('not found') && clientRunning) {
                return;
            }
            if (currentRole === 'gs') {
                setText(gsStatus, message);
            } else if (currentRole === 'client') {
                setText(clientStatus, message);
            }
            break;
        }
        case 'webrtc-ice-state': {
            const { id, state, hint } = payload;
            const iceLabel =
                state === 'connected'
                    ? 'WebRTC connected'
                    : state === 'checking'
                      ? 'Completing WebRTC handshake…'
                      : state === 'failed'
                        ? `Connection failed${hint ? ` — ${hint}` : ''}`
                        : `ICE: ${state}`;
            if (currentRole === 'gs' && id) {
                updatePeerCardStatus(id, iceLabel, state === 'connected' ? 'ok' : state === 'failed' ? '' : 'pending');
                if (state === 'connected') {
                    setText(gsStatus, `WebRTC connected to client ${id}`);
                } else if (state === 'failed') {
                    setText(gsStatus, iceLabel);
                }
            } else if (currentRole === 'client') {
                if (state === 'connected') {
                    setText(clientStatus, 'WebRTC connected to Ground Station');
                } else if (state === 'failed') {
                    setText(clientStatus, iceLabel);
                } else if (state === 'checking') {
                    setText(clientStatus, 'Completing WebRTC handshake…');
                }
            }
            break;
        }
        default:
            break;
    }
}

async function pollPendingOffers() {
    if (!gsRunning || currentRole !== 'gs') return;
    try {
        const ids = await invoke('gs_list_pending_offers');
        for (const id of ids) {
            showPendingClient(id);
        }
    } catch (err) {
        console.warn('Pending offer poll failed:', err);
    }
}

function startPendingPoll() {
    stopPendingPoll();
    pendingPollTimer = setInterval(pollPendingOffers, 1000);
}

function stopPendingPoll() {
    if (pendingPollTimer) {
        clearInterval(pendingPollTimer);
        pendingPollTimer = null;
    }
}

async function setupEventListeners() {
    const events = [
        'pending-client',
        'new-client',
        'client-removed',
        'new-gs',
        'gs-online',
        'client-error',
        'webrtc-ice-state',
        'client-mode',
        'gs-sync-full',
        'gs-sync-position',
        'gs-sync-prediction',
        'gs-disconnected',
    ];

    for (const eventName of events) {
        await listen(eventName, (event) => {
            handleConnectEvent(eventName, event.payload);
        });
    }

    window.addEventListener('message', (event) => {
        if (event.data?.type === 'harp-connect-event') {
            handleConnectEvent(event.data.event, event.data.payload);
        }
        if (event.data?.type === 'THEME_CHANGE') {
            document.documentElement.setAttribute('data-theme', event.data.theme);
        }
    });

    listenersReady = true;
}

async function applySignalingHost() {
    const raw = signalingHostInput?.value?.trim() || '127.0.0.1';
    return invoke('set_signal_server_host', { hostOrUrl: raw });
}

async function showGsSignalingHints() {
    if (!gsSignalingUrls) return;
    try {
        const hints = await invoke('get_signaling_connect_hints');
        const lines = [];
        if (hints.remote_urls?.length) {
            lines.push(...hints.remote_urls);
        } else {
            lines.push('(no LAN IP found — check network)');
        }
        lines.push(`This PC (local): ${hints.local_url}`);
        gsSignalingUrls.textContent = lines.join('\n');
    } catch (err) {
        gsSignalingUrls.textContent = 'Could not read network addresses';
        console.warn(err);
    }
}

async function startGroundStation() {
    if (!listenersReady) {
        setText(gsStatus, 'Loading…');
        await setupEventListeners();
    }

    currentRole = 'gs';
    showPanel(gsPanel);
    setText(gsStatus, 'Starting Ground Station…');
    setText(gsIdDisplay, '…');

    pendingList.replaceChildren();
    connectedClientList.replaceChildren();
    pendingEntries.clear();
    connectedEntries.clear();
    setGsConnectedStatus();

    try {
        await invoke('set_signal_server_host', { hostOrUrl: '127.0.0.1' });
        const generatedId = await invoke('gs_run');
        setText(gsIdDisplay, generatedId);
        await showGsSignalingHints();
        gsRunning = true;
        setGsPendingStatus();
        setGsConnectedStatus();
        startPendingPoll();
    } catch (error) {
        console.error('Error running Ground Station:', error);
        setText(gsStatus, `Failed to start: ${error}`);
        currentRole = null;
        showChoosePanel();
        stopPendingPoll();
    }
}

async function waitForPeerOnline(peerId, timeoutMs = 45000) {
    const start = Date.now();
    while (Date.now() - start < timeoutMs) {
        const online = await invoke('signaling_peer_online', { peerId });
        if (online) return true;
        setText(clientStatus, `Waiting for Ground Station “${peerId}” on signaling server…`);
        await new Promise((resolve) => setTimeout(resolve, 500));
    }
    return false;
}

async function startClient() {
    if (!listenersReady) {
        await setupEventListeners();
    }

    const gsId = gsIdInput.value.trim();
    if (!gsId) {
        setText(clientStatus, 'Enter a Ground Station ID first.');
        return;
    }

    currentRole = 'client';
    showPanel(clientPanel);
    setText(clientStatus, 'Checking Ground Station on signaling server…');
    setText(clientIdDisplay, '—');
    clientConnectBtn.disabled = true;
    gsIdInput.disabled = true;

    try {
        const signalUrl = await applySignalingHost();
        setText(clientStatus, `Using signaling server ${signalUrl}…`);

        const gsVisible = await waitForPeerOnline(gsId);
        if (!gsVisible) {
            let peers = [];
            try {
                peers = await invoke('list_signaling_peers');
            } catch (_) {
                /* ignore */
            }
            setText(
                clientStatus,
                `Ground Station “${gsId}” is not online. ` +
                    (peers.length
                        ? `Peers on server: ${peers.join(', ')}`
                        : 'Start Ground Station on the tracking PC first.')
            );
            clientConnectBtn.disabled = false;
            gsIdInput.disabled = false;
            currentRole = null;
            return;
        }

        setText(clientStatus, 'Ground Station found — connecting…');
        const clientName = clientNameInput?.value?.trim() || '';
        if (clientName) {
            localStorage.setItem('harp_client_name', clientName);
        }
        const generatedId = await invoke('client_run', { gsId, clientName });
        setText(clientIdDisplay, generatedId);
        clientRunning = true;
        setText(clientStatus, `Your client ID: ${generatedId} — completing handshake…`);
        if (clientNameInput) clientNameInput.disabled = true;
    } catch (error) {
        console.error('Error running Client:', error);
        setText(clientStatus, `Failed to connect: ${error}`);
        clientConnectBtn.disabled = false;
        gsIdInput.disabled = false;
        currentRole = null;
        showChoosePanel();
    }
}

function resetUi() {
    showChoosePanel();

    setText(gsIdDisplay, '—');
    if (gsSignalingUrls) gsSignalingUrls.textContent = '—';
    setText(clientIdDisplay, '—');
    setText(gsPendingStatus, 'Offers are accepted automatically when a client connects.');
    setText(clientStatus, 'Enter Ground Station details and connect.');

    pendingList.replaceChildren();
    connectedClientList.replaceChildren();
    clientGsConnection.replaceChildren();
    pendingEntries.clear();
    connectedEntries.clear();

    clientConnectBtn.disabled = false;
    gsIdInput.disabled = false;
    if (clientNameInput) clientNameInput.disabled = false;

    notifyParent('client-mode', { connected: false });

    currentRole = null;
    gsRunning = false;
    clientRunning = false;
    stopPendingPoll();
    setGsConnectedStatus();
}

async function handleDisconnect(role) {
    try {
        if (role === 'gs' && gsRunning) {
            await invoke('gs_disconnect');
        } else if (role === 'client' && clientRunning) {
            await invoke('client_disconnect');
            notifyParent('client-mode', { connected: false });
        }
    } catch (error) {
        console.error('Disconnect error:', error);
    } finally {
        resetUi();
    }
}

btnGs.addEventListener('click', startGroundStation);
btnClient.addEventListener('click', () => {
    currentRole = 'client';
    showPanel(clientPanel);
});
clientConnectBtn.addEventListener('click', startClient);
disconnectGsBtn.addEventListener('click', () => handleDisconnect('gs'));
disconnectClientBtn.addEventListener('click', () => handleDisconnect('client'));

async function loadTurnConfig() {
    const turnUrlsInput = document.getElementById('turn_urls_input');
    const turnUserInput = document.getElementById('turn_user_input');
    const turnForceRelay = document.getElementById('turn_force_relay');
    if (!turnUrlsInput) return;
    try {
        const cfg = await invoke('get_turn_config');
        turnUrlsInput.value = (cfg.turn_urls || []).join(', ');
        if (turnUserInput) turnUserInput.value = cfg.username || '';
        if (turnForceRelay) turnForceRelay.checked = !!cfg.force_relay;
    } catch (err) {
        console.warn('Could not load TURN config:', err);
    }
}

async function saveTurnConfig() {
    const status = document.getElementById('turn_save_status');
    const turnUrlsInput = document.getElementById('turn_urls_input');
    const turnUserInput = document.getElementById('turn_user_input');
    const turnCredInput = document.getElementById('turn_cred_input');
    const turnForceRelay = document.getElementById('turn_force_relay');
    if (!turnUrlsInput) return;
    try {
        await invoke('set_turn_config', {
            turnUrls: turnUrlsInput.value.trim(),
            username: turnUserInput?.value?.trim() || '',
            credential: turnCredInput?.value || '',
            forceRelay: !!turnForceRelay?.checked,
        });
        if (status) {
            status.textContent =
                'TURN saved. Disconnect and reconnect both peers for new ICE settings.';
        }
    } catch (err) {
        if (status) status.textContent = `TURN save failed: ${err}`;
    }
}

const turnSaveBtn = document.getElementById('turn_save_btn');
if (turnSaveBtn) {
    turnSaveBtn.addEventListener('click', saveTurnConfig);
}

setupCopyButtons();
showChoosePanel();

setupEventListeners()
    .then(loadTurnConfig)
    .catch((err) => {
        console.error('Failed to set up connection listeners:', err);
    });

const savedTheme = localStorage.getItem('harp-theme');
if (savedTheme) {
    document.documentElement.setAttribute('data-theme', savedTheme);
}

const savedSignalingHost = localStorage.getItem('harp_signaling_host');
if (savedSignalingHost && signalingHostInput) {
    signalingHostInput.value = savedSignalingHost;
}
const savedClientName = localStorage.getItem('harp_client_name');
if (savedClientName && clientNameInput) {
    clientNameInput.value = savedClientName;
}
if (signalingHostInput) {
    signalingHostInput.addEventListener('change', () => {
        localStorage.setItem('harp_signaling_host', signalingHostInput.value.trim());
    });
}
