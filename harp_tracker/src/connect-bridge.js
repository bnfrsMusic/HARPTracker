// forward tauri events from main window into clients.html iframe
const tauriEvent = window.__TAURI__?.event;
if (!tauriEvent) {
    console.warn('connect-bridge: Tauri event API unavailable');
} else {
    const CONNECT_EVENTS = [
        'pending-client',
        'new-client',
        'client-removed',
        'new-gs',
        'gs-online',
        'client-error',
        'webrtc-ice-state',
        'client-mode',
        'gs-sync-full',
        'gs-sync-position',
        'gs-sync-prediction',
        'gs-disconnected',
    ];

    function forwardToClientsIframe(eventName, payload) {
        const iframe = document.querySelector('iframe.clients_iframe');
        if (!iframe?.contentWindow) return;
        iframe.contentWindow.postMessage(
            { type: 'harp-connect-event', event: eventName, payload },
            '*'
        );
    }

    for (const eventName of CONNECT_EVENTS) {
        tauriEvent.listen(eventName, (event) => {
            forwardToClientsIframe(eventName, event.payload);
        });
    }
}
