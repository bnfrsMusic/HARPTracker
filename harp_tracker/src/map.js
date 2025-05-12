// Initialize the map when the DOM is loaded
document.addEventListener('DOMContentLoaded', initMap);

// Global map variable
let map = null;
let marker = null;
let firstLoad = true;
let lastLat = null;
let lastLng = null;

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
  
  // Add OpenStreetMap tile layer
  L.tileLayer('https://{s}.tile.openstreetmap.org/{z}/{x}/{y}.png', {
    attribution: '&copy; <a href="https://www.openstreetmap.org/copyright">OpenStreetMap</a> contributors',
    maxZoom: 19
  }).addTo(map);
  
  // temp location
  marker = L.marker([0, 0]).addTo(map);
  
  // Listen for messages from the parent window
  window.addEventListener('message', handleMessage);
  
  // Signal that the map is ready
  window.parent.postMessage({ type: 'MAP_READY' }, '*');
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