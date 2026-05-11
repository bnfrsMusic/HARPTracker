// DOM elements
const btn_gs     = document.getElementById('btn_gs');
const btn_node   = document.getElementById('btn_client');
const choose_panel = document.getElementById('choose_panel');
const gs_panel   = document.getElementById('gs_panel');
const c_panel    = document.getElementById('c_panel');
 
// Hide role panels on load
gs_panel.style.display = 'none';
c_panel.style.display  = 'none';
 
function assignRole() {
    btn_gs.onclick = () => {
        choose_panel.style.display = 'none';
        gs_panel.style.display     = 'flex';
    };
 
    btn_node.onclick = () => {
        choose_panel.style.display = 'none';
        c_panel.style.display      = 'flex';
    };
}
 
assignRole();