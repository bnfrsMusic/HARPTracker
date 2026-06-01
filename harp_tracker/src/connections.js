const tauriCore = window.__TAURI__?.core ?? window.parent.__TAURI__?.core;
const tauriEvent = window.__TAURI__?.event ?? window.parent.__TAURI__?.event;

if (!tauriCore || !tauriEvent) {
    console.error('Tauri API not available in this frame — open Connections from the main window.');
}

const invoke = tauriCore.invoke.bind(tauriCore);
const listen = tauriEvent.listen.bind(tauriEvent);

const btnGs = document.getElementById('btn_gs');
const btnClient = document.getElementById('btn_client');
const choosePanel = document.getElementById('choose_panel');

const gsPanel = document.getElementById('gs_panel');
const clientPanel = document.getElementById('c_panel');

const gsIdDisplay = document.getElementById('gs_id_display');
const clientIdDisplay = document.getElementById('c_id_display');

const gsStatus = document.getElementById('gs_status');
const gsPendingStatus = document.getElementById('gs_pending_status');
const clientStatus = document.getElementById('c_status');

const gsIdInput = document.getElementById('gs_id_input');
const clientConnectBtn = document.getElementById('client_accept_id');

const pendingList = document.getElementById('pending_list');
const connectedClientList = document.getElementById('client_list');
const clientGsConnection = document.getElementById('c_gs_connection');

const disconnectGsBtn = document.getElementById('btn_disconnect_gs');
const disconnectClientBtn = document.getElementById('btn_disconnect_c');

let currentRole = null;
let gsRunning = false;
let clientRunning = false;

const pendingEntries = new Map();
const connectedEntries = new Map();

gsPanel.style.display = 'none';
clientPanel.style.display = 'none';

function setGsPendingStatus() {
    const n = pendingEntries.size;
    gsPendingStatus.textContent =
        n === 0 ? 'No pending offers' : `${n} offer${n === 1 ? '' : 's'} awaiting acceptance`;
}

function setGsConnectedStatus() {
    const n = connectedEntries.size;
    gsStatus.textContent =
        n === 0
            ? 'Online — waiting for client offers'
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

function removeEntry(map, peerId, container) {
    const entry = map.get(peerId);
    if (entry) {
        entry.remove();
        map.delete(peerId);
    }
    if (container && container.childElementCount === 0) {
        container.replaceChildren();
    }
}

async function setupEventListeners() {
    await listen('pending-client', (event) => {
        if (currentRole !== 'gs') return;
        const { id } = event.payload;
        if (pendingEntries.has(id) || connectedEntries.has(id)) return;

        const entry = makePeerEntry(id, `Client offer: ${id}`, {
            indicatorClass: 'pending',
            onAccept: async () => {
                try {
                    await invoke('gs_accept_offer', { nodeId: id });
                    removeEntry(pendingEntries, id);
                    setGsPendingStatus();
                } catch (err) {
                    console.error('Accept offer failed:', err);
                    gsStatus.textContent = `Failed to accept ${id}`;
                }
            },
            onReject: async () => {
                try {
                    await invoke('gs_reject_offer', { nodeId: id });
                } catch (err) {
                    console.error('Reject offer failed:', err);
                } finally {
                    removeEntry(pendingEntries, id);
                    setGsPendingStatus();
                }
            },
        });

        pendingList.appendChild(entry);
        pendingEntries.set(id, entry);
        setGsPendingStatus();
    });

    await listen('new-client', (event) => {
        if (currentRole !== 'gs') return;
        const { id, role, name } = event.payload;
        removeEntry(pendingEntries, id);

        if (connectedEntries.has(id)) return;

        const label = name ? `${role} (${name}): ${id}` : `${role}: ${id}`;
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
    });

    await listen('client-removed', (event) => {
        if (currentRole !== 'gs') return;
        const { id } = event.payload;
        removeEntry(connectedEntries, id);
        removeEntry(pendingEntries, id);
        setGsPendingStatus();
        setGsConnectedStatus();
    });

    await listen('new-gs', (event) => {
        if (currentRole !== 'client') return;
        const { id, role } = event.payload;
        clientGsConnection.replaceChildren();
        const entry = makePeerEntry(id, `${role}: ${id}`, { indicatorClass: 'ok' });
        clientGsConnection.appendChild(entry);
        clientStatus.textContent = `Linked to ${id}`;
    });

    await listen('gs-online', (event) => {
        if (currentRole === 'gs') {
            gsStatus.textContent = `Registered on signaling server as ${event.payload.id}`;
        }
    });

    await listen('client-error', (event) => {
        const message = event.payload?.message ?? 'Connection error';
        if (currentRole === 'gs') {
            gsStatus.textContent = message;
        } else if (currentRole === 'client') {
            clientStatus.textContent = message;
        }
    });
}

async function startGroundStation() {
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
        const generatedId = await invoke('gs_run');
        gsIdDisplay.textContent = generatedId;
        gsRunning = true;
        gsStatus.textContent =
            'Online on signaling server — share your ID, then wait for offers';
        setGsPendingStatus();
    } catch (error) {
        console.error('Error running Ground Station:', error);
        gsStatus.textContent = 'Failed to start Ground Station';
        currentRole = null;
        gsPanel.style.display = 'none';
        choosePanel.style.display = 'flex';
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
                    : 'No peers are registered — start Ground Station first and keep that window open.');
            clientConnectBtn.disabled = false;
            gsIdInput.disabled = false;
            currentRole = null;
            return;
        }

        clientStatus.textContent = 'Ground Station found — connecting…';
        const generatedId = await invoke('client_run', { gs_id: gsId });
        clientIdDisplay.textContent = generatedId;
        clientRunning = true;
        clientStatus.textContent =
            'Offer sent — ensure the Ground Station is online, then wait for Accept';
    } catch (error) {
        console.error('Error running Client:', error);
        clientStatus.textContent = 'Failed to connect';
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
    clientIdDisplay.textContent = '';
    gsStatus.textContent = 'Waiting for client offers…';
    gsPendingStatus.textContent = 'No pending offers';
    clientStatus.textContent = 'Enter a Ground Station ID and connect.';

    pendingList.replaceChildren();
    connectedClientList.replaceChildren();
    clientGsConnection.replaceChildren();
    pendingEntries.clear();
    connectedEntries.clear();

    clientConnectBtn.disabled = false;
    gsIdInput.disabled = false;

    currentRole = null;
    gsRunning = false;
    clientRunning = false;
}

async function handleDisconnect(role) {
    try {
        if (role === 'gs' && gsRunning) {
            await invoke('gs_disconnect');
        } else if (role === 'client' && clientRunning) {
            await invoke('client_disconnect');
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

setupEventListeners();

window.addEventListener('message', (event) => {
    if (event.data && event.data.type === 'THEME_CHANGE') {
        document.documentElement.setAttribute('data-theme', event.data.theme);
    }
});

const savedTheme = localStorage.getItem('harp-theme');
if (savedTheme) {
    document.documentElement.setAttribute('data-theme', savedTheme);
}
