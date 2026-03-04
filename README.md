# HARP Tracker
## What is it?
HARP Tracker is a graphical user interface (GUI) designed for ballooning ground station teams. Inspired by the HERMES GUI developed by the University of Minnesota Twin Cities, the project is currently under active development by Mercer University students Ayush Sahoo and Samhita Saragadam. It uses Rust and Tauri to ensure great performance, while still acheiving memory safety.


## NOTE: In-Development Features
The following features are not available in the latest version, but will be implemented in the future
- **Connected Clients** 
    - Connection between different HARP Tracker clients for ground station operations 
- **NEBP's RFD Connectivity**
- **NEBP's Arduino Connectivity**


## How to Install
In the [tags section](https://github.com/bnfrsMusic/HARPTracker/tags), download a .exe file from the version of your interest. Once downloaded, it's ready to run!

## How to Use
### Quick Start
To quickly get tracking up and running simply follow the steps below:
- Open the `Connections` section
- Select the type of connection that you want to track. 
    - Currently the software supports APRS and Iridium, with support for WSPR and NEBP's RFD system under works. 
- Input the identification information for the connection (eg. APRS callsign, Iridium IMEI)
- Click on `Activate`
- If the identifier is valid and the software is able to retrieve information about it, the status indicator will turn green.
- The map, altitude graph, and predictions will then update accordingly and begin tracking


## Credits
Thanks to the following projects for their APIs and map layers that are crucial to this project:
- [aprs.fi](https://aprs.fi/)
- [SondeHub](https://www.sondehub.org/)


- [RainViewer](https://www.rainviewer.com/api.html)
- [OpenSky](https://opensky-network.org/)
- [Stadia Maps](https://stadiamaps.com/)

- [OpenStreetMap](https://www.openstreetmap.org/)
- [Leaflet](https://leafletjs.com/)