import { setCompassAngle, createCompass } from "./compass.js";

// allow quick dev call
window.updateInfo = updateInfo;
const { invoke } = window.__TAURI__.core;

// DOM elements
let utcMsg;
let dateMsg;
let lat, long, alt;
let last_update;
let city, state;
let console_text;
let previousLat = null;
let previousLong = null;

// Track active instances
let lastKnownPosition = null;
let moduleCatalog = [];

// Interval IDs
let utcIntervalId;
let trackerIntervalId;
let statusIntervalId;
let predictionIntervalId;

// true when receiving flight data from a ground station as client
let isGsClientMode = false;

// geocoding API rate limiting
let lastGeocodeTime = 0;
const GEOCODE_RATE_LIMIT = 10000;

// Prediction parameters
let predictionParams = {
  payloadMass: 2.0,
  balloonMass: 1.5,
  parachuteDragCoeff: 0.5,
  burstAltitude: 30000.0,
  ascentRate: null,
  descentRate: 5.0,
};

// Initialize app
async function init() {
  try {
    const sideTabs = document.querySelectorAll(".side-tab, .sidebar-tab");
    const panelContents = document.getElementById("panel-contents");
    let activeTab = null;

    sideTabs.forEach((btn) => {
      btn.addEventListener("click", () => {
        const panelId = btn.dataset.panel;
        const panel = document.getElementById(panelId);

        if (activeTab === btn) {
          btn.classList.remove("active");
          activeTab = null;
          if (panelContents) {
            panelContents.classList.remove("open");
            panelContents.setAttribute("aria-hidden", "true");
          }
          if (panel) panel.classList.remove("active");
          document.body.classList.remove("panel-open");
          return;
        }

        sideTabs.forEach((b) => b.classList.remove("active"));
        btn.classList.add("active");
        activeTab = btn;

        document
          .querySelectorAll(".panel")
          .forEach((p) => p.classList.remove("active"));
        if (panel) panel.classList.add("active");
        if (panelContents) {
          panelContents.classList.add("open");
          panelContents.setAttribute("aria-hidden", "false");
        }
        document.body.classList.add("panel-open");
      });
    });

    document.addEventListener("click", (e) => {
      const target = e.target;
      if (
        !target.closest(".panel-contents") &&
        !target.closest(".side-tab") &&
        !target.closest(".sidebar-tab") &&
        !target.closest("harp-select") &&
        !target.closest(".harp-dd-portal")
      ) {
        if (panelContents) {
          panelContents.classList.remove("open");
          panelContents.setAttribute("aria-hidden", "true");
        }
        sideTabs.forEach((b) => b.classList.remove("active"));
        activeTab = null;
        document
          .querySelectorAll(".panel")
          .forEach((p) => p.classList.remove("active"));
        document.body.classList.remove("panel-open");
      }
    });

    const addBtn = document.getElementById("add-connection");
    const list = document.getElementById("connections-list");
    if (addBtn && list) {
      await loadModuleCatalog();
      addBtn.addEventListener("click", () => addConnection(list));
      addConnection(list);
    }

    // Request Location button handler
    const requestLocationBtn = document.getElementById("request-location-btn");
    if (requestLocationBtn) {
      requestLocationBtn.addEventListener("click", async () => {
        showConsole("Requesting location...");
        
        // Try to get location from browser geolocation
        try {
          const location = await getUserLocation();
          if (location) {
            showConsole(`Location acquired: ${location.latitude.toFixed(6)}, ${location.longitude.toFixed(6)}`);
            
            // Update ground station inputs with the acquired location
            const gsLat = document.getElementById("gs-lat");
            const gsLon = document.getElementById("gs-lon");
            const gsAlt = document.getElementById("gs-alt");
            if (gsLat && gsLon && gsAlt) {
              gsLat.value = location.latitude.toFixed(6);
              gsLon.value = location.longitude.toFixed(6);
              gsAlt.value = "0"; // Default altitude
            }
            
            // Update compass with the new location
            await updateCompass(location.latitude, location.longitude);
          } else {
            showConsole("Failed to acquire location. Please enter coordinates manually in Settings.");
          }
        } catch (err) {
          showConsole("Location request failed: " + err);
        }
      });
    }

    createCompass(document.getElementById("compass-top-left"));
    window.setCompassAngle = setCompassAngle;

    try {
      async function pollHeading() {
        try {
          const heading = await invoke("get_heading");
          if (typeof heading === "number" || !Number.isNaN(Number(heading))) {
            setCompassAngle(Number(heading));
          }
        } catch (err) {}
      }
      pollHeading();
      setInterval(pollHeading, 1000);

      initThemeSelector();
    } catch (e) {}
  } catch (err) {
    console.warn("UI init warning:", err);
  }

  // Get DOM elements
  utcMsg = document.querySelector("#utc-msg");
  dateMsg = document.querySelector("#date-msg");
  lat = document.querySelector("#lat");
  long = document.querySelector("#long");
  alt = document.querySelector("#alt");
  last_update = document.querySelector("#last-update");
  city = document.querySelector("#city");
  state = document.querySelector("#state");
  console_text = document.querySelector("#console-text");

  // Setup prediction controls
  setupPredictionControls();

  const filteringMethod = document.querySelector("#filtering-method");
  if (filteringMethod) {
    filteringMethod.addEventListener("change", handleFilteringMethodChange);
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

  // Initial Updates
  await date();
  await updateTracker();
  await updateUtc();
  await updateActiveStatus();
  await updateConnectedClients();

  // Start timers
  utcIntervalId = setInterval(updateUtc, 100);
  trackerIntervalId = setInterval(updateTracker, 15000);
  statusIntervalId = setInterval(updateActiveStatus, 1000);

  // Start prediction timer (every 30 seconds)
  predictionIntervalId = setInterval(runPrediction, 30000);

  setupClientSyncListeners();
}

// Setup prediction controls
function setupPredictionControls() {
  // Get prediction panel inputs
  const payloadMassInput = document.querySelector(
    "#predictions .payload-params label:nth-child(1) input",
  );
  const balloonMassInput = document.querySelector(
    "#predictions .payload-params label:nth-child(2) input",
  );
  const parachuteDragInput = document.querySelector(
    "#predictions .payload-params label:nth-child(3) input",
  );

  // Set default values
  if (payloadMassInput) payloadMassInput.value = predictionParams.payloadMass;
  if (balloonMassInput) balloonMassInput.value = predictionParams.balloonMass;
  if (parachuteDragInput)
    parachuteDragInput.value = predictionParams.parachuteDragCoeff;

  // Add event listeners for parameter changes
  if (payloadMassInput) {
    payloadMassInput.addEventListener("change", (e) => {
      predictionParams.payloadMass = parseFloat(e.target.value) || 2.0;
      updatePredictionParams();
    });
  }

  if (balloonMassInput) {
    balloonMassInput.addEventListener("change", (e) => {
      predictionParams.balloonMass = parseFloat(e.target.value) || 1.5;
      updatePredictionParams();
    });
  }

  if (parachuteDragInput) {
    parachuteDragInput.addEventListener("change", (e) => {
      predictionParams.parachuteDragCoeff = parseFloat(e.target.value) || 0.5;
      updatePredictionParams();
    });
  }

  // Get run prediction button
  const runBtn = document.querySelector(
    "#predictions .run-controls button:first-child",
  );
  if (runBtn) {
    runBtn.addEventListener("click", async () => {
      await runPrediction();
    });
  }

  // Algorithm selector
  const algoSelect = document.querySelector("#predictions label harp-select");
  if (algoSelect) {
    algoSelect.addEventListener("change", async (e) => {
      const algorithm = e.target.value;
      try {
        await invoke("set_predictor", { name: algorithm });
        if (console_text)
          console_text.textContent = `Predictor set to: ${algorithm}`;
      } catch (error) {
        if (console_text)
          console_text.textContent = `Error setting predictor: ${error}`;
      }
    });
  }
}

// Update prediction parameters in backend
async function updatePredictionParams() {
  try {
    await invoke("set_prediction_params", {
      payloadMass: predictionParams.payloadMass,
      balloonMass: predictionParams.balloonMass,
      parachuteDragCoeff: predictionParams.parachuteDragCoeff,
      burstAltitude: predictionParams.burstAltitude,
      ascentRate: predictionParams.ascentRate,
      descentRate: predictionParams.descentRate,
    });
  } catch (error) {
    console.error("Error updating prediction params:", error);
  }
}

// Run prediction
async function runPrediction() {
  if (isGsClientMode) return;
  try {
    if (console_text) console_text.textContent = "Starting predictions...";

    // Update parameters first
    await updatePredictionParams();

    // Run prediction
    const result = await invoke("run_prediction");

    if (console_text) console_text.textContent = "Predictions complete!";

    // Send prediction data to map
    const mapIframe = document.querySelector(".screen");
    if (mapIframe && mapIframe.contentWindow) {
      mapIframe.contentWindow.postMessage(
        {
          type: "UPDATE_PREDICTION",
          data: result,
        },
        "*",
      );
    }

    console.log("Prediction result:", result);
  } catch (error) {
    if (console_text) console_text.textContent = `Prediction error: ${error}`;
    console.error("Prediction error:", error);
  }
}

//------------------------------Update Functions------------------------------
// Update date
async function date() {
  try {
    dateMsg.textContent = await invoke("date");
  } catch (error) {
    console_text.textContent = "Error updating date:" + error;
  }
}

async function updateUtc() {
  try {
    utcMsg.textContent = await invoke("utc");
    await updateLastUpdate();
  } catch (error) {
    console_text.textContent = "Error updating timing:" + error;
  }
}

async function updateTracker() {
  if (isGsClientMode) return;
  try {
    // Update tracker data
    await invoke("update");

    // Update position display
    await getPosition();

    const now = new Date();
    const timeStr = now.toLocaleTimeString("en-US", {
      hour: "2-digit",
      minute: "2-digit",
    });
    if (console_text)
      console_text.textContent = `${timeStr}: Tracker data updated`;
  } catch (error) {
    console_text.textContent = "Error in tracker update cycle:" + error;
  }
}

// Update status indicators for active services
async function updateActiveStatus() {
  if (isGsClientMode) return;
  try {
    await loadModuleCatalog();
    await updateConnectedClients();
    try {
      await updateConnectionIndicators();
    } catch (e) {}
  } catch (error) {
    console_text.textContent = "Error updating active status:" + error;
  }
}

// Update the display of connected clients
async function updateConnectedClients() {
  try {
    const snapshots = await invoke("get_module_snapshots");
    document.querySelectorAll(".connection-entry[data-module-id]").forEach((entry) => {
      const snapshot = snapshots.find((item) => item.id === entry.dataset.moduleId);
      const indicator = entry.querySelector(".conn-indicator");
      if (!indicator) return;
      indicator.classList.toggle("ok", Boolean(snapshot?.connected));
      indicator.classList.toggle("pending", Boolean(snapshot?.enabled && !snapshot?.connected));
    });
  } catch (error) {
    console.error("Error updating connected clients:", error);
  }
}

async function loadModuleCatalog() {
  moduleCatalog = await invoke("get_module_catalog");
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
    const response = await fetch(
      `https://nominatim.openstreetmap.org/reverse?format=json&lat=${latitude}&lon=${longitude}&zoom=10&addressdetails=1`,
      {
        headers: {
          "User-Agent": "HARP-Tracker-App/1.0",
        },
      },
    );

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
    let cityName =
      data.address.city ||
      data.address.town ||
      data.address.village ||
      data.address.hamlet ||
      "Unknown";

    let stateName =
      data.address.state || data.address.province || data.address.region || "";

    // Update UI
    if (citystate) citystate.textContent = cityName + ", " + stateName;

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

//for handling filtering method changes
async function handleFilteringMethodChange(event) {
  const newValue = event.target.value;
  try {
    await invoke("set_filtering_method", { method: newValue });
    if (console_text)
      console_text.textContent = "Filtering method updated: " + newValue;
  } catch (error) {
    if (console_text)
      console_text.textContent = "Error updating filtering method: " + error;
    else console.error("Error updating filtering method:", error);
  }
}

// Get and display current position
async function getPosition() {
  try {
    const currentLat = await invoke("get_lat");
    const currentLong = await invoke("get_long");
    const altitude = await invoke("get_alt");
    const horiz_vel = await invoke("get_horiz_vel");
    const vert_vel = await invoke("get_vert_vel");

    const numLat = Number(currentLat);
    const numLong = Number(currentLong);
    const numAlt = Number(altitude);
    const numHoriz = Number(horiz_vel);
    const numVert = Number(vert_vel);

    if (lat && !Number.isNaN(numLat)) lat.textContent = numLat + ",";
    if (long && !Number.isNaN(numLong)) long.textContent = numLong;
    if (alt && !Number.isNaN(numAlt)) alt.textContent = numAlt + "m";

    // Update map
    if (
      !Number.isNaN(numLat) &&
      !Number.isNaN(numLong) &&
      !Number.isNaN(numAlt)
    ) {
      updateMap(numLat, numLong, numAlt, numHoriz, numVert);

      // Check if coordinates have changed significantly before updating city
      const hasLocationChanged =
        previousLat === null ||
        previousLong === null ||
        (typeof numLat === "number" &&
          typeof previousLat === "number" &&
          Math.abs(numLat - previousLat) > 0.01) ||
        (typeof numLong === "number" &&
          typeof previousLong === "number" &&
          Math.abs(numLong - previousLong) > 0.01);

      // Update elements if location has changed
      if (hasLocationChanged) {
        if (!Number.isNaN(numLat) && !Number.isNaN(numLong)) {
          //update city and state
          updateCityAndState(numLat, numLong);

          // make UTC timestamp
          const now = new Date();
          const utcTimeStr =
            now.getUTCHours().toString().padStart(2, "0") +
            ":" +
            now.getUTCMinutes().toString().padStart(2, "0") +
            ":" +
            now.getUTCSeconds().toString().padStart(2, "0");
          //update compass
          try {
            await updateCompass(numLat, numLong);
          } catch (e) {}

          // update alt graph with the utc timestamp
          try {
            if (numAlt != 0.0){updateAltitudeGraph(utcTimeStr, numAlt);}
          } catch (e) {}
        }
        previousLat = Number.isFinite(numLat) ? numLat : previousLat;
        previousLong = Number.isFinite(numLong) ? numLong : previousLong;
      }
    }
  } catch (error) {
    if (console_text)
      console_text.textContent = "Error getting position:" + error;
    else console.error("Error getting position:", error);
  }
}

function addConnection(container) {
  const entry = document.createElement("div");
  entry.className = "connection-entry";

  const indicator = document.createElement("div");
  indicator.className = "conn-indicator";

  const type = document.createElement("harp-select");
  const emptyOption = document.createElement("harp-option");
  emptyOption.setAttribute("value", "");
  emptyOption.textContent = "Select module";
  type.appendChild(emptyOption);
  moduleCatalog.forEach((definition) => {
    const option = document.createElement("harp-option");
    option.setAttribute("value", definition.module_type);
    option.textContent = definition.display_name;
    type.appendChild(option);
  });

  const fields = document.createElement("div");
  fields.className = "connection-config-fields";

  const activate = document.createElement("button");
  activate.className = "activate";
  activate.innerText = "Activate";
  const remove = document.createElement("button");
  remove.className = "remove";
  remove.innerText = "✕";

  function selectedDefinition() {
    return moduleCatalog.find((definition) => definition.module_type === type.value);
  }

  function renderFields() {
    fields.replaceChildren();
    const definition = selectedDefinition();
    if (!definition) return;

    definition.fields.forEach((field) => {
      const input = document.createElement("input");
      input.type = field.field_type || "text";
      input.name = field.key;
      input.placeholder = field.placeholder || field.label;
      input.required = field.required;
      input.dataset.moduleField = field.key;
      fields.appendChild(input);
    });
  }

  function readConfig() {
    const config = {};
    fields.querySelectorAll("[data-module-field]").forEach((input) => {
      config[input.dataset.moduleField] = input.value.trim();
    });
    return config;
  }

  function moduleId(config) {
    const definition = selectedDefinition();
    const identityField = definition?.fields.find((field) =>
      ["id", "device_id", "call_sign", "modem"].includes(field.key),
    );
    return config[identityField?.key] || `${type.value}-${Date.now()}`;
  }

  async function activateModule() {
    const definition = selectedDefinition();
    if (!definition) {
      showConsole("Select a module first");
      return;
    }
    const config = readConfig();
    const missing = definition.fields.find((field) => field.required && !config[field.key]);
    if (missing) {
      showConsole(`Enter ${missing.label}`);
      return;
    }

    const id = moduleId(config);
    await invoke("configure_module", {
      moduleType: definition.module_type,
      moduleId: id,
      config,
    });
    entry.dataset.moduleId = id;
    type.disabled = true;
    fields.querySelectorAll("input").forEach((input) => { input.disabled = true; });
    activate.dataset.active = "1";
    activate.innerText = "Deactivate";
    indicator.classList.add("pending");
    await updateConnectedClients();
  }

  type.addEventListener("change", renderFields);
  activate.addEventListener("click", async () => {
    try {
      if (activate.dataset.active === "1") {
        await invoke("remove_module", { moduleId: entry.dataset.moduleId });
        delete entry.dataset.moduleId;
        type.disabled = false;
        fields.querySelectorAll("input").forEach((input) => { input.disabled = false; });
        activate.dataset.active = "0";
        activate.innerText = "Activate";
        indicator.classList.remove("ok", "pending");
      } else {
        await activateModule();
      }
    } catch (error) {
      showConsole(`Module error: ${error}`);
    }
  });

  remove.addEventListener("click", async () => {
    if (entry.dataset.moduleId) {
      await invoke("remove_module", { moduleId: entry.dataset.moduleId }).catch(() => {});
    }
    entry.remove();
  });

  entry.append(indicator, type, fields, activate, remove);
  container.appendChild(entry);
  type.focus();
}

//------------------------------Connection Handlers------------------------------

// The catalog-driven editor uses the same snapshot path for every module.
async function updateConnectionIndicators() {
  await updateConnectedClients();
}

//update UTC text and last-update placeholder
export function updateInfo({ utcText, lastUpdate, cityState }) {
  const u = document.getElementById("utc-msg");
  const l = document.getElementById("last-update");
  const c = document.getElementById("citystate");
  if (u && utcText) u.textContent = utcText;
  if (l && lastUpdate) l.textContent = lastUpdate;
  if (c && cityState) c.textContent = cityState;
}

//helper to show short messages in the console area
function showConsole(msg, timeout = 4000) {
  if (console_text) {
    console_text.textContent = msg;
    if (timeout > 0)
      setTimeout(() => {
        if (console_text && console_text.textContent === msg)
          console_text.textContent = "";
      }, timeout);
  } else {
    console.log(msg);
  }
}

//------------------------------Map Functions/Handlers------------------------------

//Init the map iframe
function initMapIframe() {
  const mapIframe = document.querySelector(".screen");

  // Set the iframe source to the map HTML file
  mapIframe.src = "map.html";

  window.addEventListener("message", async (event) => {
    const mapIframe = document.querySelector(".screen");

    if (event.data?.type === "MAP_READY") {
      console.log("Map is ready");

      // Send current position if we have it
      updateMapWithCurrentPosition();
      syncAircraftConfigToMap();
      return;
    }

    if (event.data?.type === "FETCH_OPENSKY") {
      const { id, lamin, lomin, lamax, lomax } = event.data;
      try {
        const data = await invoke("fetch_opensky_states", {
          lamin,
          lomin,
          lamax,
          lomax,
        });
        mapIframe?.contentWindow?.postMessage(
          { type: "OPENSKY_RESULT", id, data },
          "*",
        );
      } catch (error) {
        mapIframe?.contentWindow?.postMessage(
          {
            type: "OPENSKY_ERROR",
            id,
            error: String(error),
          },
          "*",
        );
      }
    }
  });

  const aircraftRadiusInput = document.getElementById("aircraft-radius-km");
  if (aircraftRadiusInput) {
    aircraftRadiusInput.addEventListener("change", syncAircraftConfigToMap);
    aircraftRadiusInput.addEventListener("blur", syncAircraftConfigToMap);
  }
}

function syncAircraftConfigToMap() {
  const mapIframe = document.querySelector(".screen");
  if (!mapIframe?.contentWindow) return;

  const radiusInput = document.getElementById("aircraft-radius-km");
  const radiusKm = Number(radiusInput?.value);
  const radiusMeters =
    Number.isFinite(radiusKm) && radiusKm > 0 ? radiusKm * 1000 : 100000;

  mapIframe.contentWindow.postMessage(
    {
      type: "SET_AIRCRAFT_CONFIG",
      radiusMeters,
    },
    "*",
  );
}

// Update the map with current position
async function updateMapWithCurrentPosition() {
  try {
    const currentLat = await invoke("get_lat");
    const currentLong = await invoke("get_long");
    const altitude = await invoke("get_alt");
    const horiz = await invoke("get_horiz_vel");
    const vert = await invoke("get_vert_vel");

    console.log(`Fetched velocities: H:${horiz} V:${vert}`);

    if (currentLat !== 0 || currentLong !== 0) {
      updateMap(currentLat, currentLong, altitude, horiz, vert);
    }
  } catch (error) {
    if (console_text)
      console_text.textContent =
        "Error getting position for map update:" + error;
    else console.error("Error getting position for map update:", error);
  }
}

async function updateMap(latitude, longitude, altitude, horiz_vel, vert_vel) {
  const mapIframe = document.querySelector(".screen");

  // Make sure iframe is loaded
  if (!mapIframe || !mapIframe.contentWindow) {
    console.warn("Map iframe not ready");
    return;
  }

  if (typeof horiz_vel === "undefined" || typeof vert_vel === "undefined") {
    try {
      horiz_vel = await invoke("get_horiz_vel");
      vert_vel = await invoke("get_vert_vel");
    } catch (error) {
      console.error("Error fetching velocities:", error);
      horiz_vel = 0;
      vert_vel = 0;
    }
  }
  mapIframe.contentWindow.postMessage(
    {
      type: "UPDATE_POSITION",
      lat: latitude,
      lng: longitude,
      alt: altitude,
      horiz_vel: horiz_vel,
      vert_vel: vert_vel,
    },
    "*",
  );
}

//------------------------------Compass Functions/Handlers------------------------------

//gets the user location using a couple different methods
async function getUserLocation() {
  try {
    // 1) Prefer explicit Ground Station inputs if the user puts it in the Settings panel
    const gsLat = document.getElementById("gs-lat");
    const gsLon = document.getElementById("gs-lon");
    if (gsLat && gsLon) {
      const latVal = gsLat.value && gsLat.value.trim();
      const lonVal = gsLon.value && gsLon.value.trim();
      const latNum = Number(latVal);
      const lonNum = Number(lonVal);
      if (
        !Number.isNaN(latNum) &&
        !Number.isNaN(lonNum) &&
        latVal !== "" &&
        lonVal !== ""
      ) {
        return { latitude: latNum, longitude: lonNum };
      }
    }
    // 2) Fallback to browser geolocation (wrapped as a Promise)
    return await new Promise((resolve, reject) => {
      if (!navigator.geolocation) return resolve(null);
      const options = { timeout: 7000, maximumAge: 0 };
      navigator.geolocation.getCurrentPosition(
        (pos) =>
          resolve(
            pos && pos.coords
              ? {
                  latitude: pos.coords.latitude,
                  longitude: pos.coords.longitude,
                }
              : null,
          ),
        (err) => {
          console.warn(`Geolocation error: ${err && err.message}`);
          resolve(null);
        },
        options,
      );
    });
  } catch (err) {
    console.warn("getUserLocation error:", err);
    return null;
  }
}

function angleFromCoordinate(lat1, long1, lat2, long2) {
  // compute bearing from (lat1,long1) -> (lat2,long2) in degrees (0 = north)
  const toRad = (d) => (d * Math.PI) / 180;
  const toDeg = (r) => (r * 180) / Math.PI;
  const t1 = toRad(lat1);
  const t2 = toRad(lat2);
  const delta = toRad(long2 - long1);
  const y = Math.sin(delta) * Math.cos(t2);
  const x =
    Math.cos(t1) * Math.sin(t2) - Math.sin(t1) * Math.cos(t2) * Math.cos(delta);
  let theta = Math.atan2(y, x);
  theta = toDeg(theta);
  return (theta + 360) % 360;
}

async function updateCompass(lat, long) {
  try {
    const ucoords = await getUserLocation();
    if (
      ucoords &&
      typeof ucoords.latitude === "number" &&
      typeof ucoords.longitude === "number"
    ) {
      const ulat = ucoords.latitude;
      const ulong = ucoords.longitude;
      const bearing = angleFromCoordinate(ulat, ulong, lat, long);
      if (typeof setCompassAngle === "function") setCompassAngle(bearing);
    } else {
      if (console_text)
        console_text.textContent =
          "Could not determine user location for compass";
    }
  } catch (err) {
    console.error("updateCompass error:", err);
  }
}

//------------------------------Altitude Graph------------------------------

//updates the alt graph
function updateAltitudeGraph(time, alt) {
  const iframe = document.getElementById("altitude-graph");

  iframe.contentWindow.postMessage(
    {
      type: "ADD_DATA",
      time: time,
      alt: alt,
    },
    "*",
  );
}

function formatUtcTime(unixSecs) {
  const d = new Date(unixSecs * 1000);
  return (
    d.getUTCHours().toString().padStart(2, "0") +
    ":" +
    d.getUTCMinutes().toString().padStart(2, "0") +
    ":" +
    d.getUTCSeconds().toString().padStart(2, "0")
  );
}

function loadAltitudeHistory(points) {
  const iframe = document.getElementById("altitude-graph");
  if (!iframe?.contentWindow) return;
  const entries = (points || []).map((p) => ({
    time: formatUtcTime(p.time),
    alt: p.alt,
  }));
  iframe.contentWindow.postMessage({ type: "LOAD_HISTORY", entries }, "*");
}

function setClientSyncMode(enabled) {
  isGsClientMode = enabled;
  const tracking = document.getElementById("connection-tracking");
  const predictionsTab = document.getElementById("predictions-tab");
  const predictionsPanel = document.getElementById("predictions");
  if (tracking) tracking.style.display = enabled ? "none" : "";
  if (predictionsTab) predictionsTab.style.display = enabled ? "none" : "";
  if (predictionsPanel && enabled) predictionsPanel.classList.remove("active");
  if (console_text) {
    console_text.textContent = enabled
      ? "Client mode — receiving flight data from Ground Station"
      : "";
  }
}

function applyGsSyncFull(payload) {
  const history = payload.history || [];
  const position = payload.position;
  const prediction = payload.prediction;
  const mapIframe = document.querySelector(".screen");
  if (mapIframe?.contentWindow) {
    mapIframe.contentWindow.postMessage(
      { type: "LOAD_TRACKING_HISTORY", points: history },
      "*",
    );
    if (prediction) {
      mapIframe.contentWindow.postMessage(
        { type: "UPDATE_PREDICTION", data: prediction },
        "*",
      );
    }
  }
  loadAltitudeHistory(history);
  if (position) applyGsSyncPosition({ position });
}

function applyGsSyncPosition(payload) {
  const p = payload.position;
  if (!p) return;
  lastKnownPosition = {
    lat: p.lat,
    lon: p.lon,
    alt: p.alt,
    horiz_vel: p.horiz_vel,
    vert_vel: p.vert_vel,
  };
  updateMap(p.lat, p.lon, p.alt, p.horiz_vel, p.vert_vel);
  if (p.last_update) {
    updateAltitudeGraph(formatUtcTime(p.last_update), p.alt);
  }
  if (p.lat && p.lon) {
    updateCityAndState(p.lat, p.lon).catch(() => {});
  }
}

function applyGsSyncPrediction(payload) {
  const mapIframe = document.querySelector(".screen");
  if (mapIframe?.contentWindow && payload.prediction) {
    mapIframe.contentWindow.postMessage(
      { type: "UPDATE_PREDICTION", data: payload.prediction },
      "*",
    );
  }
}

function handleClientSyncEvent(eventName, payload) {
  switch (eventName) {
    case "client-mode":
      setClientSyncMode(!!payload?.connected);
      break;
    case "gs-sync-full":
      setClientSyncMode(true);
      applyGsSyncFull(payload || {});
      break;
    case "gs-sync-position":
      applyGsSyncPosition(payload || {});
      break;
    case "gs-sync-prediction":
      applyGsSyncPrediction(payload || {});
      break;
    case "gs-disconnected":
      if (console_text) {
        console_text.textContent =
          "Connection lost — reconnecting to Ground Station…";
      }
      break;
    default:
      break;
  }
}

function setupClientSyncListeners() {
  window.addEventListener("message", (event) => {
    if (event.data?.type === "harp-connect-event") {
      handleClientSyncEvent(event.data.event, event.data.payload);
    }
  });
  const tauriEvent = window.__TAURI__?.event;
  if (!tauriEvent) return;
  for (const eventName of [
    "client-mode",
    "gs-sync-full",
    "gs-sync-position",
    "gs-sync-prediction",
    "gs-disconnected",
  ]) {
    tauriEvent.listen(eventName, (event) => {
      handleClientSyncEvent(eventName, event.payload);
    });
  }
}

function initThemeSelector() {
  const themeSelect = document.querySelector("#settings harp-select");

  if (!themeSelect) return;

  const savedTheme = localStorage.getItem("harp-theme") || "light";
  setTheme(savedTheme);
  themeSelect.value = savedTheme.charAt(0).toUpperCase() + savedTheme.slice(1);

  themeSelect.addEventListener("change", (e) => {
    const selectedValue = e.target.value.toLowerCase();
    setTheme(selectedValue);
  });
}

function setTheme(theme) {
  document.documentElement.setAttribute("data-theme", theme);
  localStorage.setItem("harp-theme", theme);
  updateIframes(theme);
}

function updateIframes(theme) {
  const iframes = document.querySelectorAll("iframe");
  iframes.forEach((iframe) => {
    iframe.contentWindow.postMessage(
      { type: "THEME_CHANGE", theme: theme },
      "*",
    );
  });
}
function cleanup() {
  if (utcIntervalId) clearInterval(utcIntervalId);
  if (trackerIntervalId) clearInterval(trackerIntervalId);
  if (statusIntervalId) clearInterval(statusIntervalId);
  if (statusIntervalId) clearInterval(statusIntervalId);
}

// init app once DOM is loaded
window.addEventListener("DOMContentLoaded", init);
// Cleanup on page unload if needed
window.addEventListener("beforeunload", cleanup);
