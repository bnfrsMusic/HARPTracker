// Simple compass: injects an SVG and exposes functions to set angle
export function createCompass(container) {
    if (!container) return;
    container.innerHTML = `
        <svg class="compass" viewBox="0 0 100 100" xmlns="http://www.w3.org/2000/svg">
            <defs>
                <style> 
                    .tick { stroke: rgba(0,0,0,0.4); stroke-width: 1.5; stroke-linecap: round; }
                    .compass-text { 
                        fill: var(--primary-color); 
                        font-family: afacad-flux, sans-serif;
                        dominant-baseline: central;
                    }
                </style>
            </defs>
            
            <text x="50" y="7" text-anchor="middle" font-size="12" font-weight="700" class="compass-text">N</text>
            <text x="93" y="50" text-anchor="middle" font-size="12" font-weight="700" class="compass-text">E</text>
            <text x="50" y="93" text-anchor="middle" font-size="12" font-weight="700" class="compass-text">S</text>
            <text x="7" y="50" text-anchor="middle" font-size="12" font-weight="700" class="compass-text">W</text>
            
            <line class="tick" x1="50" y1="18" x2="50" y2="24" transform="rotate(0 50 50)" />
            <line class="tick" x1="50" y1="18" x2="50" y2="24" transform="rotate(90 50 50)" />
            <line class="tick" x1="50" y1="18" x2="50" y2="24" transform="rotate(180 50 50)" />
            <line class="tick" x1="50" y1="18" x2="50" y2="24" transform="rotate(270 50 50)" />
            
            <line class="tick" x1="50" y1="18" x2="50" y2="22" transform="rotate(45 50 50)" />
            <line class="tick" x1="50" y1="18" x2="50" y2="22" transform="rotate(135 50 50)" />
            <line class="tick" x1="50" y1="18" x2="50" y2="22" transform="rotate(225 50 50)" />
            <line class="tick" x1="50" y1="18" x2="50" y2="22" transform="rotate(315 50 50)" />

            <line class="tick" x1="50" y1="18" x2="50" y2="21" transform="rotate(22.5 50 50)" />
            <line class="tick" x1="50" y1="18" x2="50" y2="21" transform="rotate(67.5 50 50)" />
            <line class="tick" x1="50" y1="18" x2="50" y2="21" transform="rotate(112.5 50 50)" />
            <line class="tick" x1="50" y1="18" x2="50" y2="21" transform="rotate(157.5 50 50)" />
            <line class="tick" x1="50" y1="18" x2="50" y2="21" transform="rotate(202.5 50 50)" />
            <line class="tick" x1="50" y1="18" x2="50" y2="21" transform="rotate(247.5 50 50)" />
            <line class="tick" x1="50" y1="18" x2="50" y2="21" transform="rotate(292.5 50 50)" />
            <line class="tick" x1="50" y1="18" x2="50" y2="21" transform="rotate(337.5 50 50)" />

            <g id="needle" transform="rotate(0 50 50)">
                <polygon points="50,6 47,50 50,44 53,50" fill="#c0392b" class="needle"/>
                <polygon points="50,94 47,50 50,56 53,50" fill="#2c3e50"/>
            </g>
            
            <circle cx="50" cy="50" r="2.5" fill="#000"/>
        </svg>`;
}

// function to set compass angle based on degrees passed in
export function setCompassAngle(deg){
    const needle = document.querySelector('#needle');
    if(!needle) return;
    needle.setAttribute('transform', `rotate(${deg} 50 50)`);
}