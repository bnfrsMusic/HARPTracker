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

const gs_input = document.getElementById('gs_id_input');
const c_accept_btn = document.getElementById('client_accept_id');

const c_available_gs = document.getElementById('gs_listed'); 
const gs_available_peers = document.getElementById('client_list');

const disconnect_gs = document.getElementById('btn_disconnect_gs');
const disconnect_c = document.getElementById('btn_disconnect_c');
 
// Hide role panels on load
gs_panel.style.display = 'none';
c_panel.style.display  = 'none';

gs_status.textContent = 'No Clients Online';
c_status.textContent = 'Waiting for Ground Station Connection...';

function assignRole() {
    btn_gs.onclick = async () => {
        choose_panel.style.display = 'none';
        gs_panel.style.display     = 'flex';
        gs_status.textContent = 'Ground Station Connected!';

        await listen('new-client', (event) => {
            register_c_to_gs(event.payload.id, event.payload.role);
        });

        try {
            const generatedId = await invoke('gs_run');
            gs_gen_id.textContent = generatedId;
        } catch (error) {
            console.error('Error running GS:', error);
            gs_status.textContent = 'Error Running Ground Station';
        }
    };
 
    btn_node.onclick = async () => {
        choose_panel.style.display = 'none';
        c_panel.style.display      = 'flex';
        c_status.textContent = 'Client Connected!';

        c_accept_btn.onclick = async () => {
            const gsId = gs_input.value.trim();
            if (!gsId) {
                c_status.textContent = 'Please enter a Ground Station ID.';
                return;
            }

            c_accept_btn.disabled = true;
            gs_input.disabled    = true;
            c_status.textContent    = 'Connecting...';

            try {
                const generatedId = await invoke('client_run', { gsId });
                c_gen_id.textContent = generatedId;
                c_status.textContent = 'Client Connected!';
                register_gs_to_c(gsId, "Ground Station");
            } catch (error) {
                console.error('Error running Client:', error);
                c_status.textContent    = 'Error Running Client';
                c_accept_btn.disabled = false;
                gs_input.disabled    = false;
            }
        };
    };
}

function addPeerEntry(peerId, peerRole, from) {
    const peerEntry = document.createElement('div');
    peerEntry.className = 'connection-entry';

    const indicator = document.createElement('div');
    indicator.className = 'conn-indicator';

    const idText = document.createElement('span');
    idText.textContent = peerRole + ': ' + peerId;

    peerEntry.appendChild(indicator);
    peerEntry.appendChild(idText);

    if(from === 'gs') {
        gs_available_peers.appendChild(peerEntry);
    }
    else{
        c_available_gs.appendChild(peerEntry);
    }
}

async function register_c_to_gs(id, role) {
    let c_peer = addPeerEntry(id, role, 'gs');
}

async function register_gs_to_c(id, role) {
    let gs_peer = addPeerEntry(id, role, 'c');
}

function disconnect() {
    const handleDisconnect = async (role) => {
        try {
            if (role === 'gs') {
                await invoke('gs_disconnect');
            } else {
                await invoke('client_disconnect');
            }
        } catch (error) {
            console.error('Disconnect error:', error);
        } finally {
            gs_panel.style.display     = 'none';
            c_panel.style.display      = 'none';
            choose_panel.style.display = 'flex';
            gs_gen_id.textContent      = '';
            c_gen_id.textContent       = '';
            gs_status.textContent      = 'No Clients Online';
            c_status.textContent       = 'Waiting for Ground Station Connection...';

            // Re-enable client input fields for a clean next session
            const gs_id_input    = document.getElementById('gs_id_input');
            const btn_connect_gs = gs_id_input.nextElementSibling;
            gs_id_input.disabled    = false;
            if (btn_connect_gs) btn_connect_gs.disabled = false;
        }
    };

    btn_disconnect_gs.onclick = () => handleDisconnect('gs');
    btn_disconnect_c.onclick  = () => handleDisconnect('c');
}

assignRole();
disconnect();