import { setCompassAngle, createCompass } from './compass.js';


// allow quick dev call
window.updateInfo = updateInfo;
const { invoke } = window.__TAURI__.core;

// DOM elements
let utcMsg;
let dateMsg;
let ir_mod;
let aprs_call;
let lat, long, alt;
let last_update;
let city, state;
let aprs_butt, iridium_butt;
let console_text;
let radioDropdown;
let radioInput;

// Track previous values to avoid unnecessary updates
let previousIridiumValue = "";
let previousAprsValue = "";
let previousLat = null;
let previousLong = null;

// Track active instances
let activeAprsCallsigns = [];
let activeIridiumModems = [];

// Interval IDs for clearing if needed
let utcIntervalId;
let trackerIntervalId;
let statusIntervalId;

// geocoding API calls rate limiting
let lastGeocodeTime = 0;
const GEOCODE_RATE_LIMIT = 10000; 



// Initialize app
async function init() {
  // Initialize side-tabs, add-connection and compass (UI features)
    try {
      const sideTabs = document.querySelectorAll('.side-tab, .sidebar-tab');
      const panelContents = document.getElementById('panel-contents');
      let activeTab = null;

      sideTabs.forEach(btn => {
        btn.addEventListener('click', () => {
          const panelId = btn.dataset.panel;
          const panel = document.getElementById(panelId);

          // If clicking the currently active tab -> toggle close
          if (activeTab === btn) {
            // close
            btn.classList.remove('active');
            activeTab = null;
            if (panelContents) {
              panelContents.classList.remove('open');
              panelContents.setAttribute('aria-hidden', 'true');
            }
            if (panel) panel.classList.remove('active');
            document.body.classList.remove('panel-open');
            return;
          }

          // otherwise open the clicked tab
          sideTabs.forEach(b => b.classList.remove('active'));
          btn.classList.add('active');
          activeTab = btn;

          document.querySelectorAll('.panel').forEach(p => p.classList.remove('active'));
          if (panel) panel.classList.add('active');
          if (panelContents) {
            panelContents.classList.add('open');
            panelContents.setAttribute('aria-hidden', 'false');
          }
          document.body.classList.add('panel-open');
        });
      });

      // close panel when clicking outside (but allow toggling via tabs)
      document.addEventListener('click', (e) => {
        const target = e.target;
        if (!target.closest('.panel-contents') && !target.closest('.side-tab') && !target.closest('.sidebar-tab')) {
          if (panelContents) {
            panelContents.classList.remove('open');
            panelContents.setAttribute('aria-hidden', 'true');
          }
          sideTabs.forEach(b => b.classList.remove('active'));
          activeTab = null;
          document.querySelectorAll('.panel').forEach(p => p.classList.remove('active'));
          document.body.classList.remove('panel-open');
        }
      });

      const addBtn = document.getElementById('add-connection');
      const list = document.getElementById('connections-list');
      if (addBtn && list) {
        addBtn.addEventListener('click', () => addConnection(list));
        // create a default connection
        addConnection(list);
      }

      // Compass init at top-left
      createCompass(document.getElementById('compass-top-left'));
      window.setCompassAngle = setCompassAngle;
      // Try to pull heading from backend if available and update periodically
      try {
        async function pollHeading() {
          try {
            const heading = await invoke('get_heading');
            if (typeof heading === 'number' || !Number.isNaN(Number(heading))) {
              setCompassAngle(Number(heading));
            }
          } catch (err) {
            // backend may not expose heading; ignore errors silently
          }
        }
        // initial attempt
        pollHeading();
        // poll every second
        setInterval(pollHeading, 1000);
      } catch (e) {
        // not fatal
      }
    } catch (err) {
      console.warn('UI init warning:', err);
    }
  // Get DOM elements
  utcMsg = document.querySelector("#utc-msg");
  dateMsg = document.querySelector("#date-msg");
  ir_mod = document.querySelector("#iridium_field");
  aprs_call = document.querySelector("#aprs_field");
  lat = document.querySelector("#lat");
  long = document.querySelector("#long");
  alt = document.querySelector("#alt");
  last_update = document.querySelector("#last-update");
  city = document.querySelector("#city");
  state = document.querySelector("#state");
  aprs_butt = document.querySelector("#aprs_butt");
  iridium_butt = document.querySelector("#iridium_butt");
  console_text = document.querySelector("#console-text");
  radioDropdown = document.querySelector("#radio-method");
  radioInput = document.querySelector(".dropdown input[type='text']");
  
  // filtering method dropdown
  const filteringMethod = document.querySelector("#filtering-method");
  if (filteringMethod) {
    filteringMethod.addEventListener("change", handleFilteringMethodChange);
    // Load saved
    try {
      const savedMethod = await invoke("get_filtering_method");
      if (savedMethod) {
        filteringMethod.value = savedMethod;
      }
    } catch (error) {
      console.error("Error loading filtering method:", error);
    }
  }


// Set up event listener for dropdown changes
if (radioDropdown && radioInput) {
  // Update input placeholder based on selection
  radioDropdown.addEventListener("change", handleRadioDropdownChange);
  
  // Set initial placeholder
  updateInputPlaceholder(radioDropdown.value);
  
  // Set up input field event listeners
  radioInput.addEventListener("blur", handleRadioInputBlur);
  radioInput.addEventListener("keypress", function(event) {
    if (event.key === "Enter") {
      handleRadioInputBlur(event);
    }
  });
  
  // Load saved value for current selection
  loadRadioInputValue(radioDropdown.value);
}

// Function to handle dropdown selection changes
function handleRadioDropdownChange(event) {
  const selectedRadio = event.target.value;
  updateInputPlaceholder(selectedRadio);
  loadRadioInputValue(selectedRadio);
}

function updateInputPlaceholder(radioType) {
  const radioInput = document.querySelector(".dropdown input[type='text']");
  if (!radioInput) return;
  
  if (radioType === "iridium_field") {
    radioInput.placeholder = "Enter Iridium Modem ID";
  } else if (radioType === "aprs_field") {
    radioInput.placeholder = "Enter APRS Callsign";
  }
}

async function loadRadioInputValue(radioType) {
  const radioInput = document.querySelector(".dropdown input[type='text']");
  if (!radioInput) return;
  
  try {
    if (radioType === "iridium_field") {
      const savedIridium = await invoke("get_irr_modem");
      radioInput.value = savedIridium || "";
    } else if (radioType === "aprs_field") {
      const savedAprs = await invoke("get_aprs_callsign");
      radioInput.value = savedAprs || "";
    }
  } catch (error) {
    if (console_text) console_text.textContent = "Error loading radio value: " + error;
    else console.error("Error loading radio value:", error);
  }
}

async function handleRadioInputBlur(event) {
  const radioDropdown = document.querySelector("#radio-method");
  if (!radioDropdown) return;
  
  const selectedRadio = radioDropdown.value;
  const newValue = event.target.value.trim();
  
  // Route to appropriate handler based on selection
  if (selectedRadio === "iridium_field") {
    await handleIridiumUpdate(newValue);
  } else if (selectedRadio === "aprs_field") {
    await handleAprsUpdate(newValue);
  }
  
  // Clear the input field after adding
  event.target.value = "";
}
  
  // Initialize the map iframe
  initMapIframe();
  
  // Set up event listeners for input fields
  if (ir_mod) {
    ir_mod.addEventListener("blur", handleIridiumInput);
    ir_mod.addEventListener("keypress", function(event) {
      if (event.key === "Enter") {
        handleIridiumInput(event);
      }
    });
  }

  if (aprs_call) {
    aprs_call.addEventListener("blur", handleAprsInput);
    aprs_call.addEventListener("keypress", function(event) {
      if (event.key === "Enter") {
        handleAprsInput(event);
      }
    });
  }
  

  await loadSavedValues();
  
  // Initial Updates
  await date();
  await updateTracker();
  await updateUtc();
  await updateActiveStatus();
  await updateConnectedClients();
  
  // --------------------Timers--------------------
  // UTC time and Last Update every 0.1 seconds
  utcIntervalId = setInterval(updateUtc, 100);
  
  // tracker data every 10 seconds (CHANGE THIS VALUE TO ADJUST UPDATE RATE)
  trackerIntervalId = setInterval(updateTracker, 10000);
  
  // status indicators every 2 seconds
  statusIntervalId = setInterval(updateActiveStatus, 1000);
}


// Load any saved values from the backend
async function loadSavedValues() {
  try {
    const savedIridium = await invoke("get_irr_modem");
    if (savedIridium) {
      ir_mod.value = savedIridium;

      if (ir_mod) ir_mod.value = savedIridium;

      previousIridiumValue = savedIridium;
    }
    
    const savedAprs = await invoke("get_aprs_callsign");
    if (savedAprs) {
      aprs_call.value = savedAprs;

      if (aprs_call) aprs_call.value = savedAprs;

      previousAprsValue = savedAprs;
    }
  } catch (error) {
    if (console_text) console_text.textContent = "Failed to load saved values:" + error;
    else console.error("Failed to load saved values:", error);
  }
}

//------------------------------Update Functions------------------------------
// Update date
async function date() {
  try {
    dateMsg.textContent = await invoke("date");
  } catch (error) {
    console_text.textContent = "Error updating date:" + error;
    
    // console.error("Error updating date:", error);
  }
}
// Update UTC time and last update time
async function updateUtc() {
  try {
    utcMsg.textContent = await invoke("utc");
    await updateLastUpdate();
  } catch (error) {
    console_text.textContent = "Error updating timing:" + error;

  }
}
// Update tracker data and position 
async function updateTracker() {
  try {
    // Update tracker data
    await invoke("update");
    
    // Update position display
    await getPosition();
    {
      const now = new Date();

      const timeStr = now.toLocaleTimeString('en-US', { hour: '2-digit', minute: '2-digit' });
      if (console_text) console_text.textContent = `${timeStr}: Tracker data updated`;
    }
    // console.log("Tracker data updated");
  } catch (error) {
    console_text.textContent = "Error in tracker update cycle:" + error;
    // console.error("Error in tracker update cycle:", error);
  }


}

// Update status indicators for active services
async function updateActiveStatus() {
  try {
    // Check if active and show button on the Connected Clients
    const isAprsActive = await invoke("is_aprs_active");
    if (aprs_butt) {
      if (isAprsActive) aprs_butt.style.display = "inline";
      else aprs_butt.style.display = "none";
    }
    
    try {
      const isIridiumActive = await invoke("is_iridium_active");
      if (iridium_butt) {
        if (isIridiumActive) iridium_butt.style.display = "inline";
        else iridium_butt.style.display = "none";
      }
    } catch (error) {
      console_text.textContent = "Error checking Iridium status:" + error;
      // console.error("Error checking Iridium status:", error);
    }
    
    // Update the connection display with fresh last update time
    await updateConnectedClients();
    // update connection indicator lights
    try { await updateConnectionIndicators(); } catch(e) { /* ignore */ }
  } catch (error) {
    console_text.textContent = "Error updating active status:" + error;

    // console.error("Error updating active status:", error);
  }
}

// Update the display of connected clients
async function updateConnectedClients() {
  try {
    const signalFlexbox = document.querySelector(".signal_flexbox");
    if (!signalFlexbox) return;
    
    //Clear the stuff
    const existingConnections = signalFlexbox.querySelectorAll('.connection-item');
    existingConnections.forEach(item => item.remove());
    
    //validity data
    const aprsValidity = await invoke("get_aprs_validity");
    const iridiumValidity = await invoke("get_iridium_validity");
    
    //last update time for formatting
    // const lastUpdateSeconds = await invoke("get_last_update");
    
    // Update APRS button if there are active APRS connections
    if (activeAprsCallsigns.length > 0 && aprs_butt) {
      const callsign = activeAprsCallsigns[0]; //first callsign
      aprs_butt.textContent = `APRS\n${callsign}`;
      const isValid = aprsValidity[0]; // Check valid data
      aprs_butt.style.backgroundColor = isValid ? "#90EE90" : "white"; // Green if valid, white if not
      aprs_butt.style.display = "inline";
      
      // Add additional APRS connections
      for (let i = 1; i < activeAprsCallsigns.length; i++) {
        const item = document.createElement("button");
        item.className = "connection-item";
        item.textContent = `APRS\n${activeAprsCallsigns[i]}`;
        const isValid = aprsValidity[i]; // Check if this APRS has valid data
        item.style.backgroundColor = isValid ? "#90EE90" : "white"; // Green if valid, white if not
        signalFlexbox.appendChild(item);
      }
    }
    
    // Update Iridium button if there are active Iridium connections
    if (activeIridiumModems.length > 0 && iridium_butt) {
      const modem = activeIridiumModems[0]; 
      iridium_butt.textContent = `Iridium | ${modem}`;
      const isValid = iridiumValidity[0]; 
      iridium_butt.style.backgroundColor = isValid ? "#90EE90" : "white"; // Green if valid, white if not
      iridium_butt.style.display = "inline";
      
      // Add additional Iridium connections
      for (let i = 1; i < activeIridiumModems.length; i++) {
        const item = document.createElement("button");
        item.className = "connection-item";
        item.textContent = `Iridium | ${activeIridiumModems[i]}`;
        const isValid = iridiumValidity[i]; // Check valid data
        item.style.backgroundColor = isValid ? "#90EE90" : "white"; // Green if valid, white if not
        signalFlexbox.appendChild(item);
      }
    }
  } catch (error) {
    console.error("Error updating connected clients:", error);
  }
}

// Look up city and state based on coordinates
async function updateCityAndState(latitude, longitude) {
  // Check rate limiting
  const now = Date.now();
  if (now - lastGeocodeTime < GEOCODE_RATE_LIMIT) {
    return;
  }
  
  lastGeocodeTime = now;
  
  try {
    
    const response = await fetch(`https://nominatim.openstreetmap.org/reverse?format=json&lat=${latitude}&lon=${longitude}&zoom=10&addressdetails=1`, {
      headers: {
        'User-Agent': 'HARP-Tracker-App/1.0'
      }
    });

    if (!response.ok) {
      if (response.status === 429) {
        // rate limited
        if (citystate) citystate.textContent = "Location, Rate limited";
        return;
      }
      throw new Error(`Geocoding API error: ${response.status}`);
    }

    const data = await response.json();
    
    // Extract city and state information
    let cityName = data.address.city || 
                   data.address.town || 
                   data.address.village || 
                   data.address.hamlet ||
                   "Unknown";
                   
    let stateName = data.address.state || 
                    data.address.province || 
                    data.address.region ||
                    "";
    
    // Update UI
    if (citystate) citystate.textContent = cityName + ", " + stateName;
    // if (state) state.textContent = stateName;

    console.log(`Updated location: ${cityName}, ${stateName}`);
  } catch (error) {
    console.error("Error getting city/state:", error);
    if (citystate) citystate.textContent = "Location, Unknown";
  }
}

// Update the "last update" text
async function updateLastUpdate() {
  try {
    const seconds = await invoke("get_last_update");
    if (last_update) last_update.textContent = `Last update: ${seconds}s ago`;
  } catch (error) {
    console.error("Error updating last update time:", error);
  }
}


//------------------------------Input Handlers------------------------------


// Handle Iridium input changes
async function handleIridiumUpdate(newValue) {
  try {
    if (newValue !== "") {
      await invoke("set_irr_modem", { id: newValue });
      await invoke("set_iridium");
      
      // Add to active instances list if not already present
      if (!activeIridiumModems.includes(newValue)) {
        activeIridiumModems.push(newValue);
      }
      
      if (console_text) console_text.textContent = "Iridium modem updated: " + newValue;
      else console.log("Iridium modem updated:", newValue);
      
      // Update the display of connected clients
      await updateConnectedClients();
    }
  } catch (error) {
    if (console_text) console_text.textContent = "Error updating Iridium settings: " + error;
    else console.error("Error updating Iridium settings:", error);
  }
}
//for handling filtering method changes
async function handleFilteringMethodChange(event) {
  const newValue = event.target.value;
  try {
    await invoke("set_filtering_method", { method: newValue });
    if (console_text) console_text.textContent = "Filtering method updated: " + newValue;
  } catch (error) {
    if (console_text) console_text.textContent = "Error updating filtering method: " + error;
    else console.error("Error updating filtering method:", error);
  }
}

// Handle APRS input changes
async function handleAprsUpdate(newValue) {
  try {
    if (newValue !== "") {
      await invoke("set_aprs_callsign", { id: newValue });
      await invoke("set_aprs");
      
      // Add to active instances list if not already present
      if (!activeAprsCallsigns.includes(newValue)) {
        activeAprsCallsigns.push(newValue);
      }
      
      if (console_text) console_text.textContent = "APRS callsign updated: " + newValue;
      else console.log("APRS callsign updated:", newValue);
      
      // Update the display of connected clients
      await updateConnectedClients();
    }
  } catch (error) {
    if (console_text) console_text.textContent = "Error updating APRS settings: " + error;
    else console.error("Error updating APRS settings:", error);
  }
}

// Get and display current position
async function getPosition() {
  try {
    const currentLat = await invoke("get_lat");
    const currentLong = await invoke("get_long");
    const altitude = await invoke("get_alt");
    
    const numLat = Number(currentLat);
    const numLong = Number(currentLong);
    const numAlt = Number(altitude);

    if (lat && !Number.isNaN(numLat)) lat.textContent = numLat + ",";
    if (long && !Number.isNaN(numLong)) long.textContent = numLong;
    if (alt && !Number.isNaN(numAlt)) alt.textContent = numAlt + "m";

    // Update map
    if (!Number.isNaN(numLat) && !Number.isNaN(numLong) && !Number.isNaN(numAlt) && numAlt != 0.0)  {
      updateMap(numLat, numLong, numAlt);

      // Check if coordinates have changed significantly before updating city
      const hasLocationChanged = 
        previousLat === null || 
        previousLong === null ||
        (typeof numLat === 'number' && typeof previousLat === 'number' && Math.abs(numLat - previousLat) > 0.01) ||
        (typeof numLong === 'number' && typeof previousLong === 'number' && Math.abs(numLong - previousLong) > 0.01);
      
      // Update elements if location has changed
      if (hasLocationChanged) {
        if (!Number.isNaN(numLat) && !Number.isNaN(numLong)) {
          //update city and state
          updateCityAndState(numLat, numLong);

          // make UTC timestamp 
          const now = new Date();
          const utcTimeStr = now.getUTCHours().toString().padStart(2, '0') + ":" + 
                            now.getUTCMinutes().toString().padStart(2, '0') + ":" + 
                            now.getUTCSeconds().toString().padStart(2, '0');
          //update compass
          try { await updateCompass(numLat, numLong); } catch(e){}
          
          // update alt graph with the utc timestamp
          try { updateAltitudeGraph(utcTimeStr, numAlt); } catch(e){}
        }
        previousLat = Number.isFinite(numLat) ? numLat : previousLat;
        previousLong = Number.isFinite(numLong) ? numLong : previousLong;
      }

    }
    
    



  } catch (error) {
    if (console_text) console_text.textContent = "Error getting position:" + error;
    else console.error("Error getting position:", error);
  }

}

//function that adds a connection to the tracker
function addConnection(container) {
    const entry = document.createElement('div');
    entry.className = 'connection-entry';

    const indicator = document.createElement('div');
    indicator.className = 'conn-indicator';

    const type = document.createElement('select');
    ['None','APRS','Iridium'].forEach(n => {
      const o = document.createElement('option'); o.value = n; o.textContent = n; type.appendChild(o);
    });

    const ident = document.createElement('input');
    ident.type = 'text';
    ident.placeholder = 'Identifier (callsign / IMEI)';

    const remove = document.createElement('button');
    remove.className = 'remove';
    remove.innerText = '✕';

    const activate = document.createElement('button');
    activate.className = 'activate';
    activate.innerText = 'Activate';

    // commit the connection to backend when user finishes input
    async function commitConnection() {
      const val = ident.value.trim();
      const t = type.value;
      if (!val || t === 'None') return;

      try {
        if (t === 'APRS') {
          await invoke('set_aprs_callsign', { id: val });
          await invoke('set_aprs');
          if (!activeAprsCallsigns.includes(val)) activeAprsCallsigns.push(val);
        } else if (t === 'Iridium') {
          await invoke('set_irr_modem', { id: val });
          await invoke('set_iridium');
          if (!activeIridiumModems.includes(val)) activeIridiumModems.push(val);
        }

          // trigger UI update and ask backend to refresh immediately
          await updateConnectedClients();
          try { await invoke('update'); } catch(e) {}

          // give backend a bit and then refresh indicators
          setTimeout(()=>{ updateConnectionIndicators().catch(()=>{}); }, 800);
      } catch (err) {
        if (console_text) console_text.textContent = 'Error saving connection: ' + err;
        else console.error('Error saving connection:', err);
      }
    }

    // remove handler: remove from DOM and from active lists
    remove.addEventListener('click', async () => {
      const val = ident.value.trim();
      const t = type.value;
      if (t === 'APRS') {
        const idx = activeAprsCallsigns.indexOf(val);
        if (idx >= 0) activeAprsCallsigns.splice(idx, 1);
        try { await invoke('set_aprs_callsign', { id: '' }); await invoke('set_aprs'); } catch(e){}
      } else if (t === 'Iridium') {
        const idx = activeIridiumModems.indexOf(val);
        if (idx >= 0) activeIridiumModems.splice(idx, 1);
        try { await invoke('set_irr_modem', { id: '' }); await invoke('set_iridium'); } catch(e){}
      }
      container.removeChild(entry);
      await updateConnectedClients();
      try { await invoke('update'); } catch(e) {}
      setTimeout(()=>{ updateConnectionIndicators().catch(()=>{}); }, 800);
    });

    // wire input events: commit on blur or enter
    ident.addEventListener('blur', commitConnection);
    ident.addEventListener('keypress', (e) => { if (e.key === 'Enter') commitConnection(); });
    type.addEventListener('change', () => { /* keep selection; user must enter identifier */ });

    // Activate toggle: ensures backend instance is created and tracked
    activate.addEventListener('click', async () => {
      const val = ident.value.trim();
      const t = type.value;
      if (!val || t === 'None') { showConsole('Enter identifier and select method first'); return; }
      if (activate.dataset.active === '1') {
        // deactivate
        activate.dataset.active = '0';
        activate.innerText = 'Activate';
        if (t === 'APRS') {
          const idx = activeAprsCallsigns.indexOf(val); if (idx >= 0) activeAprsCallsigns.splice(idx, 1);
          try { await invoke('set_aprs_callsign', { id: '' }); await invoke('set_aprs'); } catch(e) { console.error(e); }
        } else if (t === 'Iridium') {
          const idx = activeIridiumModems.indexOf(val); if (idx >= 0) activeIridiumModems.splice(idx, 1);
          try { await invoke('set_irr_modem', { id: '' }); await invoke('set_iridium'); } catch(e) { console.error(e); }
        }
        indicator.classList.remove('ok');
        showConsole('Deactivated ' + val);
        await updateConnectedClients();
        return;
      }

      // activate
      showConsole('Activating ' + val + '...');
      await commitConnection();
      activate.dataset.active = '1';
      activate.innerText = 'Deactivate';
      indicator.classList.add('pending');
      // let background process connect and then update indicators
      setTimeout(async () => { await updateConnectionIndicators(); indicator.classList.remove('pending'); }, 1500);
    });

    // test feature removed

    entry.appendChild(indicator);
    entry.appendChild(type);
    entry.appendChild(ident);
    entry.appendChild(activate);
    entry.appendChild(remove);

    container.appendChild(entry);
    ident.focus();
}

//------------------------------Connection Handlers------------------------------


// update indicators for all connection entries by querying backend validity
async function updateConnectionIndicators() {
  try {
    const entries = document.querySelectorAll('.connection-entry');
    if (!entries || entries.length === 0) return;

    const aprsValidity = await invoke('get_aprs_validity').catch(() => []);
    const iridiumValidity = await invoke('get_iridium_validity').catch(() => []);
    const savedAprsCallsign = await invoke('get_aprs_callsign').catch(()=>null);
    const savedIrrModem = await invoke('get_irr_modem').catch(()=>null);

    entries.forEach(entry => {
      const sel = entry.querySelector('select');
      const input = entry.querySelector('input');
      const indicator = entry.querySelector('.conn-indicator');
      if (!sel || !input || !indicator) return;
      const t = sel.value;
      const id = input.value.trim();

      // clear pending marker if any
      indicator.classList.remove('pending');
      if (t === 'APRS') {

        // Prefer exact match with the backend's stored callsign if available
        let isValid = false;
        if (savedAprsCallsign && id === savedAprsCallsign) {
          isValid = aprsValidity.some(v => v === true);
        } else if (aprsValidity.length > 0 && activeAprsCallsigns.length === aprsValidity.length) {
          const idx = activeAprsCallsigns.indexOf(id);
          isValid = (idx >= 0 && aprsValidity[idx]);
        } else {
          // fallback: if any validity true, and we have only one active entry, mark it
          if (aprsValidity.filter(Boolean).length === 1 && activeAprsCallsigns.length === 1 && activeAprsCallsigns[0] === id) isValid = true;
        }
        if (isValid) indicator.classList.add('ok'); else indicator.classList.remove('ok');
      } else if (t === 'Iridium') {
        let isValid = false;
        if (savedIrrModem && id === savedIrrModem) {
          isValid = iridiumValidity.some(v => v === true);
        } else if (iridiumValidity.length > 0 && activeIridiumModems.length === iridiumValidity.length) {
          const idx = activeIridiumModems.indexOf(id);
          isValid = (idx >= 0 && iridiumValidity[idx]);
        } else {
          if (iridiumValidity.filter(Boolean).length === 1 && activeIridiumModems.length === 1 && activeIridiumModems[0] === id) isValid = true;
        }
        if (isValid) indicator.classList.add('ok'); else indicator.classList.remove('ok');
      } else {
        indicator.classList.remove('ok');
      }
    });
  } catch (err) {
    console.error('Error updating connection indicators:', err);
  }
}

//update UTC text and last-update placeholder
export function updateInfo({utcText, lastUpdate, cityState}){
    const u = document.getElementById('utc-msg');
    const l = document.getElementById('last-update');
    const c = document.getElementById('citystate');
    if(u && utcText) u.textContent = utcText;
    if(l && lastUpdate) l.textContent = lastUpdate;
    if(c && cityState) c.textContent = cityState;
}

//helper to show short messages in the console area
function showConsole(msg, timeout=4000) {
  if (console_text) {
    console_text.textContent = msg;
    if (timeout > 0) setTimeout(()=>{ if (console_text && console_text.textContent === msg) console_text.textContent = ''; }, timeout);
  } else {
    console.log(msg);
  }
}


//------------------------------Map Functions/Handlers------------------------------


//Init the map iframe
function initMapIframe() {
  const mapIframe = document.querySelector('.screen');
  
  // Set the iframe source to the map HTML file
  mapIframe.src = 'map.html';
  
  //Listen for messages from the iframe
  window.addEventListener('message', (event) => {
    // Check if the map is ready
    if (event.data && event.data.type === 'MAP_READY') {
      console.log('Map is ready');
      
      // Send current position if we have it
      updateMapWithCurrentPosition();
    }
  });
}

// Update the map with current position
async function updateMapWithCurrentPosition() {
  try {
    const currentLat = await invoke("get_lat");
    const currentLong = await invoke("get_long");
    const altitude = await invoke("get_alt");
    
    // Only update if we have valid coordinates
    if (currentLat !== 0 || currentLong !== 0) {
      updateMap(currentLat, currentLong, altitude);
    }
  } catch (error) {
    console_text.textContent = "Error getting position for map update:" + error;

    // console.error("Error getting position for map update:", error);
  }
}

// Function to update the map with new coordinates
function updateMap(latitude, longitude, altitude) {
  const mapIframe = document.querySelector('.screen');
  
  // Make sure iframe is loaded
  if (!mapIframe || !mapIframe.contentWindow) {
    console.warn('Map iframe not ready');
    return;
  }
  
  // Send position update message to the iframe
  mapIframe.contentWindow.postMessage({
    type: 'UPDATE_POSITION',
    lat: latitude,
    lng: longitude,
    alt: altitude
  }, '*');
  
  console.log(`Map updated to: ${latitude}, ${longitude}, ${altitude}m`);
}


//------------------------------Compass Functions/Handlers------------------------------

//gets the user location using a couple different methods
async function getUserLocation(){
  try {
    // 1) Prefer explicit Ground Station inputs if the user puts it in the Settings panel
    const gsInputs = document.querySelectorAll('.ground-station input');
    if (gsInputs && gsInputs.length >= 2) {
      const latVal = gsInputs[0].value && gsInputs[0].value.trim();
      const lonVal = gsInputs[1].value && gsInputs[1].value.trim();
      const latNum = Number(latVal);
      const lonNum = Number(lonVal);
      if (!Number.isNaN(latNum) && !Number.isNaN(lonNum) && latVal !== '' && lonVal !== '') {
        return { latitude: latNum, longitude: lonNum };
      }
    }
    // 2) Fallback to browser geolocation (wrapped as a Promise)
    return await new Promise((resolve, reject) => {
      if (!navigator.geolocation) return resolve(null);
      const options = { timeout: 7000, maximumAge: 0 };
      navigator.geolocation.getCurrentPosition(
        (pos) => resolve(pos && pos.coords ? { latitude: pos.coords.latitude, longitude: pos.coords.longitude } : null),
        (err) => { console.warn(`Geolocation error: ${err && err.message}`); resolve(null); },
        options
      );
    });
  } catch (err) {
    console.warn('getUserLocation error:', err);
    return null;
  }
}

function angleFromCoordinate(lat1, long1, lat2, long2){
  // compute bearing from (lat1,long1) -> (lat2,long2) in degrees (0 = north)
  const toRad = (d) => d * Math.PI / 180;
  const toDeg = (r) => r * 180 / Math.PI;
  const t1 = toRad(lat1);
  const t2 = toRad(lat2);
  const delta = toRad(long2 - long1);
  const y = Math.sin(delta) * Math.cos(t2);
  const x = Math.cos(t1) * Math.sin(t2) - Math.sin(t1) * Math.cos(t2) * Math.cos(delta);
  let theta = Math.atan2(y, x);
  theta = toDeg(theta);
  return (theta + 360) % 360;
}

async function updateCompass(lat, long){
  try {
    const ucoords = await getUserLocation();
    if (ucoords && typeof ucoords.latitude === 'number' && typeof ucoords.longitude === 'number'){
      const ulat = ucoords.latitude;
      const ulong = ucoords.longitude;
      const bearing = angleFromCoordinate(ulat, ulong, lat, long);
      if (typeof setCompassAngle === 'function') setCompassAngle(bearing);
    } else {
      if (console_text) console_text.textContent = 'Could not determine user location for compass';
    }
  } catch (err) {
    console.error('updateCompass error:', err);
  }
}


//------------------------------Altitude Graph------------------------------

//updates the alt graph
function updateAltitudeGraph(time, alt) {
    const iframe = document.getElementById('altitude-graph');
    
    iframe.contentWindow.postMessage({
        type: 'ADD_DATA',
        time: time,
        alt: alt
    }, '*'); 
}

//------------------------------Cleanup------------------------------

// Cleanup function if needed
function cleanup() {
  if (utcIntervalId) clearInterval(utcIntervalId);
  if (trackerIntervalId) clearInterval(trackerIntervalId);
  if (statusIntervalId) clearInterval(statusIntervalId);
}




// init app once DOM is loaded
window.addEventListener("DOMContentLoaded", init);
// Cleanup on page unload if needed
window.addEventListener("beforeunload", cleanup);

