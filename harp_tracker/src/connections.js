const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

// DOM elements
const btn_gs     = document.getElementById('btn_gs');
const btn_node   = document.getElementById('btn_client');
const choose_panel = document.getElementById('choose_panel');

const gs_panel   = document.getElementById('gs_panel');
const c_panel    = document.getElementById('c_panel');

const gs_gen_id   = document.getElementById('gs_id_display');
const c_gen_id   = document.getElementById('c_id_display');

const gs_status = document.getElementById('gs_status');
const c_status  = document.getElementById('c_status');

const gs_available_peers = document.getElementById('client_list');

const disconnect_gs = document.getElementById('btn_disconnect_gs');
const disconnect_c = document.getElementById('btn_disconnect_c');
 
// Hide role panels on load
gs_panel.style.display = 'none';
c_panel.style.display  = 'none';

function assignRole() {
    btn_gs.onclick = async () => {
        choose_panel.style.display = 'none';
        gs_panel.style.display     = 'flex';

        await listen('new-node', (event) => {
            addPeerEntry(event.payload.id, event.payload.role);
        });

        try {
            const generatedId = await invoke('gs_run');
            gs_gen_id.textContent = generatedId;
        } catch (error) {
            console.error('Error running GS:', error);
            gs_status.textContent = 'Error running Ground Station';
        }
    };
 
    btn_node.onclick = async () => {
        choose_panel.style.display = 'none';
        c_panel.style.display      = 'flex';

        try {
            const generatedId = await invoke('client_run');
            gs_gen_id.textContent = generatedId;
        } catch (error) {
            console.error('Error running Client:', error);
            c_status.textContent = 'Error running Client';
        }
    };
}

function addPeerEntry(clientId, role) {
    const peerEntry = document.createElement('div');
    peerEntry.className = 'connection-entry';

    const indicator = document.createElement('div');
    indicator.className = 'conn-indicator';

    const idText = document.createElement('span');
    idText.textContent = role + ': ' + clientId;
    
    const connect = document.createElement('button');
    connect.innerText = 'Connect';

    peerEntry.appendChild(indicator);
    peerEntry.appendChild(idText);
    peerEntry.appendChild(connect);

    gs_available_peers.appendChild(peerEntry);
}

function disconnect() {
    const handleDisconnect = () => {
        gs_panel.style.display = 'none';
        c_panel.style.display  = 'none';
        choose_panel.style.display = 'flex';
        gs_gen_id.textContent = '';
        c_gen_id.textContent = '';
        gs_status.textContent = 'No Clients Online';
        c_status.textContent = 'Waiting for Ground Station Connection...';
    };

    btn_disconnect_gs.onclick = handleDisconnect;
    btn_disconnect_c.onclick = handleDisconnect;
}

assignRole();
disconnect();