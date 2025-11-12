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

// Track previous values to avoid unnecessary updates
let previousIridiumValue = "";
let previousAprsValue = "";
let previousLat = null;
let previousLong = null;

// Interval IDs for clearing if needed
let utcIntervalId;
let trackerIntervalId;
let statusIntervalId;

// geocoding API calls rate limiting
let lastGeocodeTime = 0;
const GEOCODE_RATE_LIMIT = 10000; // 10 seconds between calls



// Initialize app
async function init() {
  // Get DOM elements
  utcMsg = document.querySelector("#utc-msg");
  dateMsg = document.querySelector("#date-msg");
  ir_mod = document.querySelector("#iridium-field");
  aprs_call = document.querySelector("#aprs-field");
  lat = document.querySelector("#lat");
  long = document.querySelector("#long");
  alt = document.querySelector("#alt");
  last_update = document.querySelector("#last-update");
  city = document.querySelector("#city");
  state = document.querySelector("#state");
  aprs_butt = document.querySelector("#aprs_butt");
  iridium_butt = document.querySelector("#iridium_butt");
  console_text = document.querySelector("#console-text");
  
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
  
  // --------------------Timers--------------------
  // UTC time and Last Update every 0.1 seconds
  utcIntervalId = setInterval(updateUtc, 100);
  
  // tracker data every 5 seconds
  trackerIntervalId = setInterval(updateTracker, 6000);
  
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
// Update tracker data and position - runs every 5 seconds
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
    // Check if active and update the button color
    const isAprsActive = await invoke("is_aprs_active");
    if (isAprsActive) {
      aprs_butt.style.backgroundColor = "#4CAF50"; 
    } else {
      aprs_butt.style.backgroundColor = "white"; 
    }
    
    try {
      const isIridiumActive = await invoke("is_iridium_active");
      if (isIridiumActive) {
        iridium_butt.style.backgroundColor = "#4CAF50"; 
      } else {
        iridium_butt.style.backgroundColor = "white"; 
      }
    } catch (error) {
      console_text.textContent = "Error checking Iridium status:" + error;
      // console.error("Error checking Iridium status:", error);
    }
  } catch (error) {
    console_text.textContent = "Error updating active status:" + error;

    // console.error("Error updating active status:", error);
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
    if (city) city.textContent = cityName + ",";
    if (state) state.textContent = stateName;

    console.log(`Updated location: ${cityName}, ${stateName}`);
  } catch (error) {
    console.error("Error getting city/state:", error);
    if (city) city.textContent = "Location";
    if (state) state.textContent = "Unknown";
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
async function handleIridiumInput(event) {
  const newValue = event.target.value.trim();
  
  // Only update if the value has actually changed
  if (newValue !== previousIridiumValue) {
    previousIridiumValue = newValue;
    
    try {
      if (newValue !== "") {
        await invoke("set_irr_modem", { id: newValue });
        await invoke("set_iridium");

        if (console_text) console_text.textContent = "Iridium modem updated:" + newValue;
        else console.log("Iridium modem updated:", newValue);
        
      }
    } catch (error) {
      if (console_text) console_text.textContent = "Error updating Iridium settings:" + error;
      else console.error("Error updating Iridium settings:", error);
    }
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
async function handleAprsInput(event) {
  const newValue = event.target.value.trim();
  
  //update if the value has actually changed
  if (newValue !== previousAprsValue) {
    previousAprsValue = newValue;
    
    try {
      if (newValue !== "") {
        await invoke("set_aprs_callsign", { id: newValue });
        await invoke("set_aprs");
        if (console_text) console_text.textContent = "APRS callsign updated:" + newValue;
        else console.log("APRS callsign updated:", newValue);

      }
    } catch (error) {
      if (console_text) console_text.textContent = "Error updating APRS settings:" + error;
      else console.error("Error updating APRS settings:", error);
    }
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

    // Update the map with new pos
    if (!Number.isNaN(numLat) && !Number.isNaN(numLong)) {
      updateMap(numLat, numLong, numAlt);
    }
    // Update the map with the new position
    updateMap(currentLat, currentLong, altitude);
    
    // Check if coordinates have changed significantly before updating city
    const hasLocationChanged = 
      previousLat === null || 
      previousLong === null ||
      (typeof numLat === 'number' && typeof previousLat === 'number' && Math.abs(numLat - previousLat) > 0.01) ||
      (typeof numLong === 'number' && typeof previousLong === 'number' && Math.abs(numLong - previousLong) > 0.01);
    
    // Update city and state if location has changed
    if (hasLocationChanged) {
      if (!Number.isNaN(numLat) && !Number.isNaN(numLong)) updateCityAndState(numLat, numLong);
      previousLat = Number.isFinite(numLat) ? numLat : previousLat;
      previousLong = Number.isFinite(numLong) ? numLong : previousLong;
    }

    //Update altitude bar
    if (!Number.isNaN(numAlt)) {
      const altitudeFt = Math.round(numAlt * 3.28084); 
      if (typeof updateAltitudeBar === 'function') updateAltitudeBar(altitudeFt);
    }


  } catch (error) {
    if (console_text) console_text.textContent = "Error getting position:" + error;
    else console.error("Error getting position:", error);
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