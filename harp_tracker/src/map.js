// Initialize the map when the DOM is loaded
document.addEventListener('DOMContentLoaded', initMap);

// Global map variable
let map = null;
let marker = null;
let firstLoad = true;
let lastLat = null;
let lastLng = null;
let aircraftLayer = null;
let aircraftMarkers = new Map();
let aircraftFetchInterval = null;

// initialize the map
function initMap() {
  // Create the map container if it doesn't exist
  if (!document.getElementById('map')) {
    const mapDiv = document.createElement('div');
    mapDiv.id = 'map';
    mapDiv.style.width = '100%';
    mapDiv.style.height = '100%';
    document.body.appendChild(mapDiv);
  }
  
  map = L.map('map').setView([0, 0], 2);

  // Base layers
  const osm = L.tileLayer('https://{s}.tile.openstreetmap.org/{z}/{x}/{y}.png', {
    attribution: '&copy; <a href="https://www.openstreetmap.org/copyright">OpenStreetMap</a> contributors',
    maxZoom: 19
  });

  // Satellite layer (Esri World Imagery)
  const satellite = L.tileLayer('https://server.arcgisonline.com/ArcGIS/rest/services/World_Imagery/MapServer/tile/{z}/{y}/{x}', {
    attribution: 'Tiles &copy; Esri &mdash; Source: Esri, i-cubed, USDA, USGS, AEX, GeoEye, Getmapping, Aerogrid, IGN, IGP, UPR-EGP, and the GIS User Community',
    maxZoom: 19
  });

  // Add default base layer
  osm.addTo(map);
  
  // temp location marker for tracked object
  marker = L.marker([0, 0]).addTo(map);

  // Reset-to-balloon control: centers map on the balloon marker
  const ResetControl = L.Control.extend({
    options: { position: 'topleft' },
    onAdd: function () {
      const container = L.DomUtil.create('div', 'leaflet-bar leaflet-control');
      const btn = L.DomUtil.create('a', '', container);
      btn.href = '#';
      btn.title = 'Center map on balloon';
      btn.innerHTML = '◯';
      btn.style.display = 'flex';
      btn.style.alignItems = 'center';
      btn.style.justifyContent = 'center';
      btn.style.width = '34px';
      btn.style.height = '34px';

      L.DomEvent.disableClickPropagation(btn);
      L.DomEvent.on(btn, 'click', (e) => {
        L.DomEvent.stopPropagation(e);
        L.DomEvent.preventDefault(e);
        resetToBalloon();
      });

      return container;
    }
  });

  map.addControl(new ResetControl());

  // Aircraft layer group (initially empty)
  aircraftLayer = L.layerGroup().addTo(map);

  // Add layers control so user can toggle Satellite and aircraft
  const baseLayers = {
    'OpenStreetMap': osm,
    'Satellite': satellite
  };

  const overlays = {
    'Aircraft (ADS-B / OpenSky)': aircraftLayer
  };

  L.control.layers(baseLayers, overlays, {collapsed: false}).addTo(map);
  
  // Listen for messages from the parent window
  window.addEventListener('message', handleMessage);
  
  // Signal that the map is ready
  window.parent.postMessage({ type: 'MAP_READY' }, '*');

  // Start fetching aircraft periodically
  startAircraftUpdates();
}

// Handle messages from the parent window
function handleMessage(event) {
  const data = event.data;
  if (data && data.type === 'UPDATE_POSITION') {
    updateMapPosition(data.lat, data.lng, data.alt);
  }
}

// Update map position
function updateMapPosition(lat, lng, alt) {
  if (!map || !marker) return;
  
  // make sure coordinates are legit
  if (isNaN(lat) || isNaN(lng) || !isFinite(lat) || !isFinite(lng)) {
    console.error('Invalid coordinates:', lat, lng);
    return;
  }
  

  if (lat !== lastLat || lng !== lastLng) {
    marker.setLatLng([lat, lng]);
    
    // Update popup content with coordinates and altitude
    marker.bindPopup(`<b>Balloon Position</b><br>Lat: ${lat}<br>Lng: ${lng}<br>Alt: ${alt}m`).openPopup();
    
    // Center the map on the marker if this is the first load or if coordinates have changed significantly
    if (firstLoad || Math.abs(lat - lastLat) > 0.01 || Math.abs(lng - lastLng) > 0.01) {
      map.setView([lat, lng], 10);
      firstLoad = false;
    }
    
    lastLat = lat;
    lastLng = lng;
  }
}


window.updateMapPosition = updateMapPosition;

// Programmatic reset function; center the map on the balloon marker if it has a valid position
function resetToBalloon() {
  if (!map || !marker) return;
  try {
    const latlng = marker.getLatLng();
    if (!latlng || isNaN(latlng.lat) || isNaN(latlng.lng)) return;
    map.setView([latlng.lat, latlng.lng], Math.max(map.getZoom(), 10));
    marker.openPopup();
  } catch (err) {
    console.error('Failed to reset map to balloon:', err);
  }
}

window.resetToBalloon = resetToBalloon;

// ---------------------- Aircraft (ADS-B/OpenSky) Layer ----------------------

function startAircraftUpdates() {
  // Fetch immediately and then every 10 seconds
  fetchAircraftInView();
  if (aircraftFetchInterval) clearInterval(aircraftFetchInterval);
  aircraftFetchInterval = setInterval(fetchAircraftInView, 10000);
}

function stopAircraftUpdates() {
  if (aircraftFetchInterval) clearInterval(aircraftFetchInterval);
}

async function fetchAircraftInView() {
  if (!map) return;

  const bounds = map.getBounds();
  // OpenSky expects lamin, lomin, lamax, lomax as query params
  const lamin = bounds.getSouth();
  const lamax = bounds.getNorth();
  const lomin = bounds.getWest();
  const lomax = bounds.getEast();

  const url = `https://opensky-network.org/api/states/all?lamin=${lamin}&lomin=${lomin}&lamax=${lamax}&lomax=${lomax}`;

  try {
    const resp = await fetch(url);
    if (!resp.ok) {
      console.warn('OpenSky API returned non-ok:', resp.status);
      return;
    }
    const data = await resp.json();
    const states = data.states || [];

    const seen = new Set();

    for (const s of states) {
      const icao = s[0];
      const callsign = (s[1] || '').trim();
      const lon = Number(s[5]);
      const lat = Number(s[6]);
      const alt = Number(s[7]);
      const heading = Number(s[10]) || 0;

      if (!icao || Number.isNaN(lat) || Number.isNaN(lon)) continue;
      seen.add(icao);

      if (aircraftMarkers.has(icao)) {
        // update existing marker
        const m = aircraftMarkers.get(icao);
        m.setLatLng([lat, lon]);
        if (typeof m.setRotationAngle === 'function') m.setRotationAngle(heading);
        if (m.getPopup()) m.setPopupContent(`<b>${callsign || icao}</b><br>Alt: ${isFinite(alt)?Math.round(alt)+' m':'N/A'}`);
      } else {
        // create a new rotated marker using a simple airplane SVG icon
        const planeIcon = L.divIcon({
          className: 'plane-icon',
          html: `<svg width="28" height="28" viewBox="0 0 24 24" xmlns="http://www.w3.org/2000/svg"><path fill="#d00" d="M21 16v-2l-8-5V3.5a1.5 1.5 0 0 0-3 0V9l-8 5v2l8-2.5V21l-2 1v1l3-0.5L12 21v-7.5L21 16z"/></svg>`,
          iconSize: [28,28],
          iconAnchor: [14,14]
        });

        const newMarker = L.marker([lat, lon], {icon: planeIcon, rotationAngle: heading, rotationOrigin: 'center'});
        newMarker.bindPopup(`<b>${callsign || icao}</b><br>Alt: ${isFinite(alt)?Math.round(alt)+' m':'N/A'}`);
        newMarker.addTo(aircraftLayer);
        aircraftMarkers.set(icao, newMarker);
      }
    }

    // Remove markers no longer in view
    for (const [icao, marker] of aircraftMarkers.entries()) {
      if (!seen.has(icao)) {
        aircraftLayer.removeLayer(marker);
        aircraftMarkers.delete(icao);
      }
    }

  } catch (err) {
    console.error('Error fetching aircraft from OpenSky:', err);
  }
}