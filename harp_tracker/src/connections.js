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
const clientStatus = document.getElementById('c_status');

const gsIdInput = document.getElementById('gs_id_input');
const clientNameInput = document.getElementById('client_name_input');

const pendingList = document.getElementById('pending_list');
const connectedClientList = document.getElementById('client_list');
const clientGsConnection = document.getElementById('c_gs_connection');

const clientConnectBtn = document.getElementById('client_accept_id');
const disconnectGsBtn = document.getElementById('btn_disconnect_gs');
const disconnectClientBtn = document.getElementById('btn_disconnect_c');

function notifyParent(eventName, payload) {
    if (window.parent && window.parent !== window) {
        window.parent.postMessage({ type: 'harp-connect-event', event: eventName, payload }, '*');
    }
}

let currentRole = null;
let gsRunning = false;
let clientRunning = false;
let listenersReady = false;
let pendingPollTimer = null;

const pendingEntries = new Map();
const connectedEntries = new Map();

gsPanel.style.display = 'none';
clientPanel.style.display = 'none';

function setGsPendingStatus() {
    const n = pendingEntries.size;
    gsPendingStatus.textContent =
        n === 0 ? 'Offers are accepted automatically' : `${n} handshake${n === 1 ? '' : 's'} in progress…`;
}

function setGsConnectedStatus() {
    const n = connectedEntries.size;
    gsStatus.textContent =
        n === 0
            ? 'Online — waiting for clients'
            : `Connected to ${n} client${n === 1 ? '' : 's'}`;
}

function makePeerEntry(peerId, label, options = {}) {
    const { onAccept, onReject, onRemove, indicatorClass = 'pending' } = options;

    const entry = document.createElement('div');
    entry.className = 'connection-entry';
    entry.dataset.peerId = peerId;

    const indicator = document.createElement('div');
    indicator.className = `conn-indicator ${indicatorClass}`;

    const idText = document.createElement('span');
    idText.className = 'peer-label';
    idText.textContent = label;

    entry.appendChild(indicator);
    entry.appendChild(idText);

    const actions = document.createElement('div');
    actions.className = 'peer-actions';

    if (onAccept) {
        const acceptBtn = document.createElement('button');
        acceptBtn.type = 'button';
        acceptBtn.className = 'peer-action accept';
        acceptBtn.textContent = 'Accept';
        acceptBtn.addEventListener('click', onAccept);
        actions.appendChild(acceptBtn);
    }

    if (onReject) {
        const rejectBtn = document.createElement('button');
        rejectBtn.type = 'button';
        rejectBtn.className = 'peer-action reject';
        rejectBtn.textContent = 'Reject';
        rejectBtn.addEventListener('click', onReject);
        actions.appendChild(rejectBtn);
    }

    if (onRemove) {
        const removeBtn = document.createElement('button');
        removeBtn.type = 'button';
        removeBtn.className = 'peer-action remove';
        removeBtn.textContent = 'Remove';
        removeBtn.addEventListener('click', onRemove);
        actions.appendChild(removeBtn);
    }

    if (actions.childElementCount > 0) {
        entry.appendChild(actions);
    }

    return entry;
}

function removeEntry(map, peerId) {
    const entry = map.get(peerId);
    if (entry) {
        entry.remove();
        map.delete(peerId);
    }
}

function showPendingClient(id) {
    if (currentRole !== 'gs' || pendingEntries.has(id) || connectedEntries.has(id)) {
        return;
    }

    const entry = makePeerEntry(id, `Connecting: ${id}`, {
        indicatorClass: 'pending',
    });

    pendingList.appendChild(entry);
    pendingEntries.set(id, entry);
    setGsPendingStatus();
    gsStatus.textContent = `Client ${id} connecting…`;
}

function showNewClient({ id, role, name }) {
    if (currentRole !== 'gs') return;
    removeEntry(pendingEntries, id);
    if (connectedEntries.has(id)) return;

    const label = name
        ? `${name} — ${id}`
        : `${role}: ${id}`;
    const entry = makePeerEntry(id, label, {
        indicatorClass: 'ok',
        onRemove: async () => {
            try {
                await invoke('gs_remove_client', { nodeId: id });
            } catch (err) {
                console.error('Remove client failed:', err);
                gsStatus.textContent = `Failed to remove ${id}`;
            }
        },
    });

    connectedClientList.appendChild(entry);
    connectedEntries.set(id, entry);
    setGsPendingStatus();
    setGsConnectedStatus();
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
                clientGsConnection.replaceChildren();
                const gsLabel = payload.name
                    ? `${payload.name} (${payload.id})`
                    : `${payload.role}: ${payload.id}`;
                const entry = makePeerEntry(payload.id, gsLabel, { indicatorClass: 'ok' });
                clientGsConnection.appendChild(entry);
                clientStatus.textContent = `Receiving data from ${payload.id}`;
                notifyParent('client-mode', { connected: true });
            }
            break;
        case 'client-mode':
            if (currentRole === 'client') {
                notifyParent('client-mode', payload);
                if (payload.connected) {
                    clientStatus.textContent = 'Connected — receiving flight data';
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
            if (currentRole === 'gs') {
                gsStatus.textContent = `Registered on signaling server as ${payload.id}`;
            }
            break;
        case 'client-error': {
            const message = payload?.message ?? 'Connection error';
            if (message.includes('not found') && clientRunning) {
                return;
            }
            if (currentRole === 'gs') {
                gsStatus.textContent = message;
            } else if (currentRole === 'client') {
                clientStatus.textContent = message;
            }
            break;
        }
        case 'webrtc-ice-state': {
            const { id, state, hint } = payload;
            let text = `ICE ${state} (${id})`;
            if (hint) text += ` — ${hint}`;
            if (currentRole === 'gs') {
                if (state === 'connected') {
                    gsStatus.textContent = `WebRTC connected to ${id}`;
                } else if (state === 'failed') {
                    gsStatus.textContent = text;
                } else if (state === 'checking') {
                    gsStatus.textContent = `Completing WebRTC handshake with ${id}…`;
                }
            } else if (currentRole === 'client') {
                if (state === 'connected') {
                    clientStatus.textContent = `WebRTC connected to Ground Station`;
                } else if (state === 'failed') {
                    clientStatus.textContent = text;
                } else if (state === 'checking') {
                    clientStatus.textContent = 'Completing WebRTC handshake…';
                }
            }
            break;
        }
        default:
            break;
    }
}

async function pollPendingOffers() {
    if (!gsRunning || currentRole !== 'gs') {
        return;
    }
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
    const url = await invoke('set_signal_server_host', { hostOrUrl: raw });
    return url;
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
        gsStatus.textContent = 'Loading connection UI…';
        await setupEventListeners();
    }

    currentRole = 'gs';
    choosePanel.style.display = 'none';
    gsPanel.style.display = 'flex';
    gsStatus.textContent = 'Starting Ground Station…';
    gsIdDisplay.textContent = '…';

    pendingList.replaceChildren();
    connectedClientList.replaceChildren();
    pendingEntries.clear();
    connectedEntries.clear();

    try {
        await invoke('set_signal_server_host', { hostOrUrl: '127.0.0.1' });
        const generatedId = await invoke('gs_run');
        gsIdDisplay.textContent = generatedId;
        await showGsSignalingHints();
        gsRunning = true;
        gsStatus.textContent = 'Online — clients connect automatically';
        setGsPendingStatus();
        startPendingPoll();
    } catch (error) {
        console.error('Error running Ground Station:', error);
        gsStatus.textContent = `Failed to start Ground Station: ${error}`;
        currentRole = null;
        gsPanel.style.display = 'none';
        choosePanel.style.display = 'flex';
        stopPendingPoll();
    }
}

async function waitForPeerOnline(peerId, timeoutMs = 45000) {
    const start = Date.now();
    while (Date.now() - start < timeoutMs) {
        const online = await invoke('signaling_peer_online', { peerId });
        if (online) {
            return true;
        }
        clientStatus.textContent = `Waiting for Ground Station "${peerId}" on signaling server…`;
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
        clientStatus.textContent = 'Enter a Ground Station ID first.';
        return;
    }

    currentRole = 'client';
    choosePanel.style.display = 'none';
    clientPanel.style.display = 'flex';
    clientStatus.textContent = 'Checking Ground Station on signaling server…';
    clientIdDisplay.textContent = '…';
    clientConnectBtn.disabled = true;
    gsIdInput.disabled = true;

    try {
        const signalUrl = await applySignalingHost();
        clientStatus.textContent = `Using signaling server ${signalUrl}…`;

        const gsVisible = await waitForPeerOnline(gsId);
        if (!gsVisible) {
            let peers = [];
            try {
                peers = await invoke('list_signaling_peers');
            } catch (_) {
                /* ignore */
            }
            clientStatus.textContent =
                `Ground Station "${gsId}" is not online. ` +
                (peers.length
                    ? `Peers currently registered: ${peers.join(', ')}`
                    : 'No peers registered — start Ground Station first and keep that window open.');
            clientConnectBtn.disabled = false;
            gsIdInput.disabled = false;
            currentRole = null;
            return;
        }

        clientStatus.textContent = 'Ground Station found — connecting…';
        const clientName = clientNameInput?.value?.trim() || '';
        if (clientName) {
            localStorage.setItem('harp_client_name', clientName);
        }
        const generatedId = await invoke('client_run', { gsId, clientName });
        clientIdDisplay.textContent = generatedId;
        clientRunning = true;
        clientStatus.textContent = `Connecting as ${generatedId}…`;
        if (clientNameInput) clientNameInput.disabled = true;
    } catch (error) {
        console.error('Error running Client:', error);
        clientStatus.textContent = `Failed to connect: ${error}`;
        clientConnectBtn.disabled = false;
        gsIdInput.disabled = false;
        currentRole = null;
        clientPanel.style.display = 'none';
        choosePanel.style.display = 'flex';
    }
}

function resetUi() {
    choosePanel.style.display = 'flex';
    gsPanel.style.display = 'none';
    clientPanel.style.display = 'none';

    gsIdDisplay.textContent = '';
    if (gsSignalingUrls) gsSignalingUrls.textContent = '';
    clientIdDisplay.textContent = '';
    gsStatus.textContent = 'Waiting for clients…';
    gsPendingStatus.textContent = 'Offers are accepted automatically';
    clientStatus.textContent = 'Enter a Ground Station ID and connect.';

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
    choosePanel.style.display = 'none';
    clientPanel.style.display = 'flex';
    currentRole = 'client';
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
                'TURN saved. Disconnect and reconnect both peers for new ICE settings to apply.';
        }
    } catch (err) {
        if (status) status.textContent = `TURN save failed: ${err}`;
    }
}

const turnSaveBtn = document.getElementById('turn_save_btn');
if (turnSaveBtn) {
    turnSaveBtn.addEventListener('click', saveTurnConfig);
}

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
