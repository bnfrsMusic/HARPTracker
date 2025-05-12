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
  
  // Initialize the map iframe
  initMapIframe();
  
  // Set up event listeners for input fields
  ir_mod.addEventListener("blur", handleIridiumInput);
  ir_mod.addEventListener("keypress", function(event) {
    if (event.key === "Enter") {
      handleIridiumInput(event);
    }
  });
  
  aprs_call.addEventListener("blur", handleAprsInput);
  aprs_call.addEventListener("keypress", function(event) {
    if (event.key === "Enter") {
      handleAprsInput(event);
    }
  });
  

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
      previousIridiumValue = savedIridium;
    }
    
    const savedAprs = await invoke("get_aprs_callsign");
    if (savedAprs) {
      aprs_call.value = savedAprs;
      previousAprsValue = savedAprs;
    }
  } catch (error) {
    console_text.textContent = "Failed to load saved values:" + error;
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
    console_text.textContent = "Tracker data updated";
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
    city.textContent = cityName + ",";
    state.textContent = stateName;
    
    console.log(`Updated location: ${cityName}, ${stateName}`);
  } catch (error) {
    console.error("Error getting city/state:", error);
    city.textContent = "Location";
    state.textContent = "Unknown";
  }
}

// Update the "last update" text
async function updateLastUpdate() {
  try {
    const seconds = await invoke("get_last_update");
    last_update.textContent = `Last update: ${seconds}s ago`;
  } catch (error) {
    console.error("Error updating last update time:", error);
  }
}

//Update the Altitude Bar
function updateAltitudeBar(altitudeFt) {
  const maxAlt = 110_000;
  const minY =    0;
  const maxY =  396;
  
  // clamp
  const clamped = Math.max(0, Math.min(maxAlt, altitudeFt));
  // linear interpolation
  const pct = clamped / maxAlt;
  const y = maxY - pct * (maxY - minY);

  const marker = document.getElementById("altitude-marker");
  if (marker) {
    marker.setAttribute("y", y);
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
        console_text.textContent = "Iridium modem updated:" + newValue;
        
      }
    } catch (error) {
      console_text.textContent = "Error updating Iridium settings:" + error;
    }
  }
}
// Handle APRS input changes
async function handleAprsInput(event) {
  const newValue = event.target.value.trim();
  
  // Only update if the value has actually changed
  if (newValue !== previousAprsValue) {
    previousAprsValue = newValue;
    
    try {
      if (newValue !== "") {
        await invoke("set_aprs_callsign", { id: newValue });
        await invoke("set_aprs");
        console_text.textContent = "APRS callsign updated:" + newValue;

      }
    } catch (error) {
      console_text.textContent = "Error updating APRS settings:" + error;
    }
  }
}
// Get and display current position
async function getPosition() {
  try {
    const currentLat = await invoke("get_lat");
    const currentLong = await invoke("get_long");
    const altitude = await invoke("get_alt");
    
    lat.textContent = currentLat + ",";
    long.textContent = currentLong;
    alt.textContent = altitude + "m";
    
    // Update the map with the new position
    updateMap(currentLat, currentLong, altitude);
    
    // Check if coordinates have changed significantly before updating city
    const hasLocationChanged = 
      previousLat === null || 
      previousLong === null ||
      Math.abs(currentLat - previousLat) > 0.01 ||
      Math.abs(currentLong - previousLong) > 0.01;
    
    // Update city and state if location has changed
    if (hasLocationChanged) {
      updateCityAndState(currentLat, currentLong);
      previousLat = currentLat;
      previousLong = currentLong;
    }

    //Update altitude bar
    const altitudeFt = Math.round(altitude * 3.28084); 
    updateAltitudeBar(altitudeFt);


  } catch (error) {
    console_text.textContent = "Error getting position:" + error;
  }

}

//------------------------------Map Functions/Handlers------------------------------


// Initialize the map iframe
function initMapIframe() {
  const mapIframe = document.querySelector('.screen');
  
  // Set the iframe source to our map HTML file
  mapIframe.src = 'map.html';
  
  // Listen for messages from the iframe
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

// Cleanup function if needed (e.g., when component unmounts)
function cleanup() {
  if (utcIntervalId) clearInterval(utcIntervalId);
  if (trackerIntervalId) clearInterval(trackerIntervalId);
  if (statusIntervalId) clearInterval(statusIntervalId);
}




// init app once DOM is loaded
window.addEventListener("DOMContentLoaded", init);
// Cleanup on page unload if needed
window.addEventListener("beforeunload", cleanup);