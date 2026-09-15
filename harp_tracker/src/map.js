// Initialize the map when the DOM is loaded
document.addEventListener("DOMContentLoaded", initMap);

// Global map variable
let map = null;
let marker = null;
let firstLoad = true;
let lastLat = null;
let lastLng = null;
let aircraftLayer = null;
let aircraftMarkers = new Map();
let aircraftFetchInterval = null;
let aircraftMapMoveHandler = null;
let openskyRequestId = 0;
const openskyPending = new Map();
let trackingPolyline = null;
let trackingUpdateInterval = null;
let aircraftRadiusMeters = 100000; // default 100 km
let weatherLayers = {};

// RainViewer variables
let rainviewerLayer = null;
let rainviewerTimestamp = null;
let rainviewerUpdateInterval = null;

// Prediction visualization variables
let predictionLayer = null;
let burstMarker = null;
let landingMarker = null;
let ascentLine = null;
let descentLine = null;

// initialize the map
function initMap() {
  // Create the map container if it doesn't exist
  if (!document.getElementById("map")) {
    const mapDiv = document.createElement("div");
    mapDiv.id = "map";
    mapDiv.style.width = "100%";
    mapDiv.style.height = "100%";
    document.body.appendChild(mapDiv);
  }

  map = L.map("map").setView([0, 0], 2);

  // Base layers
  const osm = L.tileLayer(
    "https://{s}.tile.openstreetmap.org/{z}/{x}/{y}.png",
    {
      attribution:
        '&copy; <a href="https://www.openstreetmap.org/copyright">OpenStreetMap</a> contributors',
      maxZoom: 19,
    },
  );

  // Dark mode layer (I like my eyes, sorry)
  // Stadia Maps Alidade Smooth Dark
  const darkMode = L.tileLayer(
    "https://tiles.stadiamaps.com/tiles/alidade_smooth_dark/{z}/{x}/{y}{r}.png",
    {
      maxZoom: 20,
      attribution:
        '© <a href="https://stadiamaps.com/">Stadia Maps</a>, © <a href="https://openmaptiles.org/">OpenMapTiles</a> © <a href="http://openstreetmap.org/copyright">OpenStreetMap</a> contributors',
    },
  );

  // Satellite layer (Esri World Imagery)
  const satellite = L.tileLayer(
    "https://server.arcgisonline.com/ArcGIS/rest/services/World_Imagery/MapServer/tile/{z}/{y}/{x}",
    {
      attribution:
        "Tiles &copy; Esri &mdash; Source: Esri, i-cubed, USDA, USGS, AEX, GeoEye, Getmapping, Aerogrid, IGN, IGP, UPR-EGP, and the GIS User Community",
      maxZoom: 19,
    },
  );

  // Add default base layer
  osm.addTo(map);

  // temp location marker for tracked object
  marker = L.marker([0, 0]).addTo(map);

  // Reset-to-balloon control: centers map on the balloon marker
  const ResetControl = L.Control.extend({
    options: { position: "topleft" },
    onAdd: function () {
      const container = L.DomUtil.create("div", "leaflet-bar leaflet-control");
      const btn = L.DomUtil.create("a", "", container);
      btn.href = "#";
      btn.title = "Center map on balloon";
      btn.innerHTML = "◯";
      btn.style.display = "flex";
      btn.style.alignItems = "center";
      btn.style.justifyContent = "center";
      btn.style.width = "34px";
      btn.style.height = "34px";

      L.DomEvent.disableClickPropagation(btn);
      L.DomEvent.on(btn, "click", (e) => {
        L.DomEvent.stopPropagation(e);
        L.DomEvent.preventDefault(e);
        resetToBalloon();
      });

      return container;
    },
  });

  map.addControl(new ResetControl());

  // Add weather legend control
  const WeatherLegend = L.Control.extend({
    options: { position: "bottomright" },
    onAdd: function () {
      const container = L.DomUtil.create("div", "weather-legend");
      container.style.backgroundColor = "rgba(255, 255, 255, 0.9)";
      container.style.padding = "10px";
      container.style.borderRadius = "5px";
      container.style.boxShadow = "0 0 15px rgba(0,0,0,0.2)";
      container.style.display = "none"; // Hidden by default
      container.id = "weather-legend";

      container.innerHTML = `
        <div style="font-weight: bold; margin-bottom: 8px; font-size: 14px;">Precipitation Intensity</div>
        <div style="display: flex; align-items: center; margin: 4px 0;">
          <div style="width: 20px; height: 15px; background: #00FFFF; margin-right: 8px;"></div>
          <span style="font-size: 12px;">Light</span>
        </div>
        <div style="display: flex; align-items: center; margin: 4px 0;">
          <div style="width: 20px; height: 15px; background: #00FF00; margin-right: 8px;"></div>
          <span style="font-size: 12px;">Moderate</span>
        </div>
        <div style="display: flex; align-items: center; margin: 4px 0;">
          <div style="width: 20px; height: 15px; background: #FFFF00; margin-right: 8px;"></div>
          <span style="font-size: 12px;">Heavy</span>
        </div>
        <div style="display: flex; align-items: center; margin: 4px 0;">
          <div style="width: 20px; height: 15px; background: #FF0000; margin-right: 8px;"></div>
          <span style="font-size: 12px;">Very Heavy</span>
        </div>
        <div style="display: flex; align-items: center; margin: 4px 0;">
          <div style="width: 20px; height: 15px; background: #FF00FF; margin-right: 8px;"></div>
          <span style="font-size: 12px;">Extreme</span>
        </div>
      `;

      L.DomEvent.disableClickPropagation(container);
      return container;
    },
  });

  map.addControl(new WeatherLegend());

  // Aircraft layer group (initially empty but not added to map by default)
  aircraftLayer = L.layerGroup();

  // Initialize RainViewer layer (will be populated when enabled)
  rainviewerLayer = L.layerGroup();
  weatherLayers["Precipitation Radar (RainViewer)"] = rainviewerLayer;

  // Create prediction layer
  predictionLayer = L.layerGroup();

  // Add layers control so user can toggle Satellite and aircraft
  const baseLayers = {
    OpenStreetMap: osm,
    "Dark Mode": darkMode,
    Satellite: satellite,
  };

  // Merge overlays into a single plain object
  const overlays = { 'Aircraft (ADS-B / OpenSky)': aircraftLayer };
  for (const k in weatherLayers) {
    if (Object.prototype.hasOwnProperty.call(weatherLayers, k)) {
      overlays[k] = weatherLayers[k];
    }
  }

  for (const k in weatherLayers) {
    if (Object.prototype.hasOwnProperty.call(weatherLayers, k)) {
      overlays[k] = weatherLayers[k];
    }
  }

  L.control.layers(baseLayers, overlays, { collapsed: false }).addTo(map);

  map.on("overlayadd", function (e) {
    if (e.name === "Precipitation Radar (RainViewer)") {
      startRainViewerUpdates();
      const legend = document.getElementById("weather-legend");
      if (legend) legend.style.display = "block";
    }
    if (e.name === "Aircraft (ADS-B / OpenSky)") {
      attachAircraftMapListeners();
      fetchAircraftInView();
    }
  });

  map.on("overlayremove", function (e) {
    if (e.name === "Precipitation Radar (RainViewer)") {
      stopRainViewerUpdates();
      const legend = document.getElementById("weather-legend");
      if (legend) legend.style.display = "none";
    }
    if (e.name === "Aircraft (ADS-B / OpenSky)") {
      detachAircraftMapListeners();
      aircraftMarkers.forEach((marker, icao) => {
        aircraftLayer.removeLayer(marker);
      });
      aircraftMarkers.clear();
    }
  });

  window.addEventListener("message", handleMessage);
  window.parent.postMessage({ type: "MAP_READY" }, "*");

  startAircraftUpdates();
  loadTrackingHistory();
  startTrackingHistoryUpdates();

  // Add prediction layer to map by default
  predictionLayer.addTo(map);
}

function handleMessage(event) {
  const data = event.data;
  if (data && data.type === "UPDATE_POSITION") {
    updateMapPosition(
      data.lat,
      data.lng,
      data.alt,
      data.horiz_vel,
      data.vert_vel,
    );
  } else if (data && data.type === "UPDATE_PREDICTION") {
    updatePrediction(data.data);
  } else if (data && data.type === "LOAD_TRACKING_HISTORY") {
    loadTrackingHistoryFromPoints(data.points || []);
  } else if (data && data.type === "SET_AIRCRAFT_CONFIG") {
    if (typeof data.radiusMeters === "number" && data.radiusMeters > 0) {
      aircraftRadiusMeters = data.radiusMeters;
    }
    if (map && map.hasLayer(aircraftLayer)) {
      fetchAircraftInView();
    }
  } else if (data && data.type === "OPENSKY_RESULT" && data.id != null) {
    const pending = openskyPending.get(data.id);
    if (pending) {
      clearTimeout(pending.timeout);
      openskyPending.delete(data.id);
      pending.resolve(data.data);
    }
  } else if (data && data.type === "OPENSKY_ERROR" && data.id != null) {
    const pending = openskyPending.get(data.id);
    if (pending) {
      clearTimeout(pending.timeout);
      openskyPending.delete(data.id);
      pending.reject(new Error(data.error || "OpenSky request failed"));
    }
  }
}

function updatePrediction(predictionData) {
  console.log("Updating prediction on map:", predictionData);

  // Clear existing prediction visualizations
  if (ascentLine) predictionLayer.removeLayer(ascentLine);
  if (descentLine) predictionLayer.removeLayer(descentLine);
  if (burstMarker) predictionLayer.removeLayer(burstMarker);
  if (landingMarker) predictionLayer.removeLayer(landingMarker);

  // Draw ascent line (blue)
  if (predictionData.ascent && predictionData.ascent.length > 0) {
    const ascentPoints = predictionData.ascent.map((p) => [p.lat, p.lon]);
    ascentLine = L.polyline(ascentPoints, {
      color: "#0066FF",
      weight: 3,
      opacity: 0.7,
      dashArray: "5, 10",
    });
    ascentLine.bindPopup("<b>Predicted Ascent Path</b>");
    ascentLine.addTo(predictionLayer);
  }

  // Draw descent line (orange)
  if (predictionData.descent && predictionData.descent.length > 0) {
    const descentPoints = predictionData.descent.map((p) => [p.lat, p.lon]);
    descentLine = L.polyline(descentPoints, {
      color: "#FF6600",
      weight: 3,
      opacity: 0.7,
      dashArray: "5, 10",
    });
    descentLine.bindPopup("<b>Predicted Descent Path</b>");
    descentLine.addTo(predictionLayer);
  }

  // Add burst marker (circle)
  if (predictionData.burst) {
    const burstIcon = L.divIcon({
      className: "burst-marker",
      html: '<div style="width: 10px; height: 10px; border-radius: 50%; background-color: #a3a3a3; border: 3px solid white; box-shadow: 0 0 5px rgba(0,0,0,0.5);"></div>',
      iconSize: [20, 20],
      iconAnchor: [10, 10],
    });

    burstMarker = L.marker(
      [predictionData.burst.lat, predictionData.burst.lon],
      { icon: burstIcon },
    );
    burstMarker.bindPopup(`
      <b>Predicted Burst</b><br>
      Lat: ${predictionData.burst.lat.toFixed(4)}<br>
      Lon: ${predictionData.burst.lon.toFixed(4)}<br>
      Alt: ${predictionData.burst.alt.toFixed(1)}m
    `);
    burstMarker.addTo(predictionLayer);
  }

  // Add landing marker (X)
  if (predictionData.landing) {
    const landingIcon = L.divIcon({
      className: "landing-marker",
      html: '<div style="font-size: 24px; font-weight: bold; color: #FF0000; text-shadow: 0 0 3px white, 0 0 3px white;">✕</div>',
      iconSize: [24, 24],
      iconAnchor: [12, 12],
    });

    landingMarker = L.marker(
      [predictionData.landing.lat, predictionData.landing.lon],
      { icon: landingIcon },
    );
    landingMarker.bindPopup(`
      <b>Predicted Landing</b><br>
      Lat: ${predictionData.landing.lat.toFixed(4)}<br>
      Lon: ${predictionData.landing.lon.toFixed(4)}<br>
      Alt: ${predictionData.landing.alt.toFixed(1)}m
    `);
    landingMarker.addTo(predictionLayer);
  }

  console.log("Prediction visualization updated");
}

function setAircraftRadius(meters) {
  aircraftRadiusMeters = Number(meters) || aircraftRadiusMeters;
}

function toggleWeatherLayer(layerName, enabled) {
  if (!weatherLayers || !weatherLayers[layerName]) return;
  const layer = weatherLayers[layerName];
  if (enabled) {
    if (!layer._map) layer.addTo(map);
  } else {
    if (layer._map) map.removeLayer(layer);
  }
}

window.setAircraftRadius = setAircraftRadius;
window.toggleWeatherLayer = toggleWeatherLayer;

function getGmapsLink(lat, lon, zoom = 15) {
  const coordinates = `${lat},${lon}`;
  return `https://www.google.com/maps/search/?api=1&query=${coordinates}`;
}

function updateMapPosition(lat, lng, alt, horiz_vel = 0, vert_vel = 0) {
  if (!map || !marker) return;

  if (isNaN(lat) || isNaN(lng) || !isFinite(lat) || !isFinite(lng)) {
    console.error("Invalid coordinates:", lat, lng);
    return;
  }

  const horizVel = isFinite(horiz_vel) ? horiz_vel : 0.0;
  const vertVel = isFinite(vert_vel) ? vert_vel : 0.0;

  console.log("Updating map with velocities - H:", horizVel, "V:", vertVel);

  if (lat !== lastLat || lng !== lastLng) {
    const gMapsUrl = getGmapsLink(lat, lng);
    marker.setLatLng([lat, lng]);

    marker
      .bindPopup(
        `
          <b>Balloon Position</b><br>
          Lat: ${lat.toFixed(4)}<br>
          Lng: ${lng.toFixed(4)}<br>
          Alt: ${alt.toFixed(1)}m<br>
          H Vel: ${horizVel.toFixed(2)} m/s<br>
          V Vel: ${vertVel.toFixed(2)} m/s<br>
          <hr>
          <a href="${gMapsUrl}" target="_blank" style="color: #4285F4; font-weight: bold; text-decoration: none;">
            📍 Open in GMaps
          </a>
        `,
      )
      .openPopup();

    if (
      firstLoad ||
      Math.abs(lat - lastLat) > 0.01 ||
      Math.abs(lng - lastLng) > 0.01
    ) {
      map.setView([lat, lng], 10);
      firstLoad = false;
    }

    lastLat = lat;
    lastLng = lng;

    if (map && map.hasLayer(aircraftLayer)) {
      fetchAircraftInView();
    }
  } else {
    const gMapsUrl = getGmapsLink(lat, lng);
    marker.setPopupContent(`
          <b>Balloon Position</b><br>
          Lat: ${lat.toFixed(4)}<br>
          Lng: ${lng.toFixed(4)}<br>
          Alt: ${alt.toFixed(1)}m<br>
          H Vel: ${horizVel.toFixed(2)} m/s<br>
          V Vel: ${vertVel.toFixed(2)} m/s<br>
          <hr>
          <a href="${gMapsUrl}" target="_blank" style="color: #4285F4; font-weight: bold; text-decoration: none;">
            📍 Open in GMaps
          </a>
        `);
  }
}

window.updateMapPosition = updateMapPosition;

function resetToBalloon() {
  if (!map || !marker) return;
  try {
    const latlng = marker.getLatLng();
    if (!latlng || isNaN(latlng.lat) || isNaN(latlng.lng)) return;
    map.setView([latlng.lat, latlng.lng], Math.max(map.getZoom(), 10));
    marker.openPopup();
  } catch (err) {
    console.error("Failed to reset map to balloon:", err);
  }
}

window.resetToBalloon = resetToBalloon;

// ---------------------- RainViewer ----------------------

function startRainViewerUpdates() {
  updateRainViewer();
  if (rainviewerUpdateInterval) clearInterval(rainviewerUpdateInterval);
  rainviewerUpdateInterval = setInterval(updateRainViewer, 600000);
}

function stopRainViewerUpdates() {
  if (rainviewerUpdateInterval) clearInterval(rainviewerUpdateInterval);
  rainviewerLayer.clearLayers();
}

async function updateRainViewer() {
  try {
    const response = await fetch(
      "https://api.rainviewer.com/public/weather-maps.json",
    );
    if (!response.ok) {
      console.warn("RainViewer API returned non-ok:", response.status);
      return;
    }

    const data = await response.json();

    if (data.radar && data.radar.past && data.radar.past.length > 0) {
      const mostRecent = data.radar.past[data.radar.past.length - 1];
      const host = data.host || "https://tilecache.rainviewer.com";
      const path = mostRecent.path;
      if (!path) {
        console.warn("RainViewer frame missing path field");
        return;
      }

      rainviewerLayer.clearLayers();

      const radarTileLayer = L.tileLayer(
        `${host}${path}/256/{z}/{x}/{y}/2/1_1.png`,
        {
          attribution:
            '&copy; <a href="https://www.rainviewer.com">RainViewer</a>',
          opacity: 0.6,
          maxZoom: 12,
          maxNativeZoom: 7,
          tileSize: 256,
          zIndex: 1000,
        },
      );

      radarTileLayer.addTo(rainviewerLayer);
      console.log("RainViewer radar updated:", path);
    } else {
      console.warn("No radar data available from RainViewer");
    }
  } catch (err) {
    console.error("Error fetching RainViewer data:", err);
  }
}

// ---------------------- Tracking History ----------------------

async function loadTrackingHistory() {
  try {
    const invoke = getTauriInvoke();
    if (!invoke) {
      console.warn("Tauri invoke unavailable for tracking history");
      return;
    }
    const trackingPoints = await invoke("get_tracking_history");
    loadTrackingHistoryFromPoints(trackingPoints);
  } catch (err) {
    console.error("Error loading tracking history:", err);
  }
}

function loadTrackingHistoryFromPoints(trackingPoints) {
  if (trackingPoints && trackingPoints.length > 0) {
    const latlngs = trackingPoints.map((point) => [point.lat, point.lon]);

    if (trackingPolyline) {
      map.removeLayer(trackingPolyline);
    }

    trackingPolyline = L.polyline(latlngs, {
      color: "#FF0000",
      weight: 3,
      opacity: 0.7,
      lineJoin: "round",
      lineCap: "round",
      className: "tracking-path",
    }).addTo(map);

    const last = trackingPoints[trackingPoints.length - 1];
    if (last) {
      updateMapPosition(last.lat, last.lon, last.alt, 0, 0);
    }

    console.log(`Tracking line created with ${trackingPoints.length} points`);
  }
}

function startTrackingHistoryUpdates() {
  if (trackingUpdateInterval) clearInterval(trackingUpdateInterval);
  trackingUpdateInterval = setInterval(loadTrackingHistory, 10000);
}

// ---------------------- Aircraft ----------------------

function startAircraftUpdates() {
  fetchAircraftInView();
  if (aircraftFetchInterval) clearInterval(aircraftFetchInterval);
  aircraftFetchInterval = setInterval(fetchAircraftInView, 20000);
}

function stopAircraftUpdates() {
  if (aircraftFetchInterval) clearInterval(aircraftFetchInterval);
}

function attachAircraftMapListeners() {
  if (!map || aircraftMapMoveHandler) return;
  aircraftMapMoveHandler = () => {
    if (map.hasLayer(aircraftLayer) && !hasValidPayloadPosition()) {
      fetchAircraftInView();
    }
  };
  map.on("moveend", aircraftMapMoveHandler);
  map.on("zoomend", aircraftMapMoveHandler);
}

function detachAircraftMapListeners() {
  if (!map || !aircraftMapMoveHandler) return;
  map.off("moveend", aircraftMapMoveHandler);
  map.off("zoomend", aircraftMapMoveHandler);
  aircraftMapMoveHandler = null;
}

function hasValidPayloadPosition() {
  if (lastLat === null || lastLng === null) return false;
  if (!Number.isFinite(lastLat) || !Number.isFinite(lastLng)) return false;
  if (Math.abs(lastLat) < 0.0001 && Math.abs(lastLng) < 0.0001) return false;
  return true;
}

function getAircraftQueryBounds() {
  if (hasValidPayloadPosition()) {
    return {
      mode: "radius",
      center: { lat: lastLat, lng: lastLng },
      ...bboxFromCenterRadius(lastLat, lastLng, aircraftRadiusMeters),
    };
  }

  const bounds = map.getBounds();
  return {
    mode: "viewport",
    center: null,
    lamin: bounds.getSouth(),
    lamax: bounds.getNorth(),
    lomin: bounds.getWest(),
    lomax: bounds.getEast(),
  };
}

function fetchOpenSkyStatesViaParent(lamin, lomin, lamax, lomax) {
  return new Promise((resolve, reject) => {
    const id = ++openskyRequestId;
    const timeout = setTimeout(() => {
      openskyPending.delete(id);
      reject(new Error("OpenSky request timed out"));
    }, 20000);

    openskyPending.set(id, { resolve, reject, timeout });
    window.parent.postMessage(
      {
        type: "FETCH_OPENSKY",
        id,
        lamin,
        lomin,
        lamax,
        lomax,
      },
      "*",
    );
  });
}

function getTauriInvoke() {
  const core =
    window.__TAURI__?.core ?? window.parent?.__TAURI__?.core ?? null;
  return core?.invoke?.bind(core) ?? null;
}

function bboxFromCenterRadius(lat, lng, radiusMeters) {
  const latDelta = radiusMeters / 111320;
  const lngDelta =
    radiusMeters / (111320 * Math.cos((lat * Math.PI) / 180) || 1);
  return {
    lamin: lat - latDelta,
    lamax: lat + latDelta,
    lomin: lng - lngDelta,
    lomax: lng + lngDelta,
  };
}

function formatAircraftAltitude(baroAlt, geoAlt) {
  const alt = Number.isFinite(baroAlt)
    ? baroAlt
    : Number.isFinite(geoAlt)
      ? geoAlt
      : null;
  return alt !== null ? `${Math.round(alt)} m` : "N/A";
}

function aircraftPopupContent(icao, callsign, baroAlt, geoAlt) {
  const identifier = callsign ? `${callsign} (${icao})` : icao;
  const altText = formatAircraftAltitude(baroAlt, geoAlt);
  return `<b>${identifier}</b><br>Altitude: ${altText}`;
}

async function fetchOpenSkyStates(lamin, lomin, lamax, lomax) {
  if (window.parent && window.parent !== window) {
    try {
      return await fetchOpenSkyStatesViaParent(lamin, lomin, lamax, lomax);
    } catch (err) {
      console.warn("OpenSky parent bridge failed, trying direct invoke:", err);
    }
  }

  const invoke = getTauriInvoke();
  if (invoke) {
    return invoke("fetch_opensky_states", {
      lamin,
      lomin,
      lamax,
      lomax,
    });
  }

  const url = `https://opensky-network.org/api/states/all?lamin=${lamin}&lomin=${lomin}&lamax=${lamax}&lomax=${lomax}`;
  const resp = await fetch(url);
  if (!resp.ok) {
    throw new Error(`OpenSky API returned ${resp.status}`);
  }
  return resp.json();
}

async function fetchAircraftInView() {
  if (!map) return;

  if (!map.hasLayer(aircraftLayer)) {
    return;
  }

  const query = getAircraftQueryBounds();
  const { lamin, lamax, lomin, lomax, mode, center } = query;

  try {
    const data = await fetchOpenSkyStates(lamin, lomin, lamax, lomax);
    const states = data.states || [];

    const seen = new Set();
    const filterCenter = mode === "radius" ? center : null;

    for (const s of states) {
      const icao = s[0];
      const callsign = (s[1] || "").trim();
      const lon = Number(s[5]);
      const lat = Number(s[6]);
      const baroAlt = Number(s[7]);
      const geoAlt = Number(s[13]);
      const heading = Number(s[10]) || 0;

      if (!icao || Number.isNaN(lat) || Number.isNaN(lon)) continue;

      if (filterCenter && typeof aircraftRadiusMeters === "number") {
        try {
          const dist = map.distance(
            [lat, lon],
            [filterCenter.lat, filterCenter.lng],
          );
          if (dist > aircraftRadiusMeters) {
            if (aircraftMarkers.has(icao)) {
              const m = aircraftMarkers.get(icao);
              aircraftLayer.removeLayer(m);
              aircraftMarkers.delete(icao);
            }
            continue;
          }
        } catch (err) {}
      }

      seen.add(icao);
      const popupHtml = aircraftPopupContent(icao, callsign, baroAlt, geoAlt);

      if (aircraftMarkers.has(icao)) {
        const m = aircraftMarkers.get(icao);
        m.setLatLng([lat, lon]);
        if (typeof m.setRotationAngle === "function")
          m.setRotationAngle(heading);
        if (m.getPopup()) m.setPopupContent(popupHtml);
      } else {
        const planeIcon = L.divIcon({
          className: "plane-icon",
          html: `<svg width="28" height="28" viewBox="0 0 24 24" xmlns="http://www.w3.org/2000/svg"><path fill="#d00" d="M21 16v-2l-8-5V3.5a1.5 1.5 0 0 0-3 0V9l-8 5v2l8-2.5V21l-2 1v1l3-0.5L12 21v-7.5L21 16z"/></svg>`,
          iconSize: [28, 28],
          iconAnchor: [14, 14],
        });

        const newMarker = L.marker([lat, lon], {
          icon: planeIcon,
          rotationAngle: heading,
          rotationOrigin: "center",
        });
        newMarker.bindPopup(popupHtml);
        newMarker.addTo(aircraftLayer);
        aircraftMarkers.set(icao, newMarker);
      }
    }

    for (const [icao, marker] of aircraftMarkers.entries()) {
      if (!seen.has(icao)) {
        aircraftLayer.removeLayer(marker);
        aircraftMarkers.delete(icao);
      }
    }
  } catch (err) {
    console.error("Error fetching aircraft from OpenSky:", err);
  }
}
