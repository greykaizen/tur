/**
 * tur Browser Extension - Background Service Worker
 * 
 * Handles Native Messaging with tur-host for format fetching
 * and communication with content script
 * 
 * @author greykaizen
 */

const TUR_HOST_NAME = 'com.greykaizen.tur';
const TUR_PROTOCOL = 'tur://';

let nativePort = null;
const pendingRequests = new Map(); // requestId -> { resolve, reject, tabId }

// ============================================================================
// Initialization
// ============================================================================

chrome.runtime.onInstalled.addListener(() => {
    chrome.contextMenus.create({
        id: 'download-with-tur',
        title: 'Download with tur',
        contexts: ['link', 'video', 'audio', 'image']
    });

    console.log('[tur] Extension installed');
});

// ============================================================================
// Native Messaging
// ============================================================================

function connectNativeHost() {
    if (nativePort) {
        return nativePort;
    }

    console.log('[tur] Connecting to native host:', TUR_HOST_NAME);

    try {
        nativePort = chrome.runtime.connectNative(TUR_HOST_NAME);

        nativePort.onMessage.addListener((msg) => {
            console.log('[tur] Native message:', msg);
            handleNativeMessage(msg);
        });

        nativePort.onDisconnect.addListener(() => {
            const error = chrome.runtime.lastError;
            console.log('[tur] Native host disconnected:', error?.message);
            nativePort = null;

            // Reject all pending requests
            for (const [id, { reject }] of pendingRequests) {
                reject(new Error('Native host disconnected'));
            }
            pendingRequests.clear();
        });

        // Test connection with ping
        nativePort.postMessage({ action: 'ping' });

        return nativePort;
    } catch (e) {
        console.error('[tur] Failed to connect:', e);
        nativePort = null;
        return null;
    }
}

function handleNativeMessage(msg) {
    switch (msg.type) {
        case 'pong':
            console.log('[tur] Connected to tur-host v' + msg.version);
            break;

        case 'formats':
            // Find pending request and resolve it
            for (const [id, request] of pendingRequests) {
                if (request.url === msg.url) {
                    request.resolve(msg.formats);
                    pendingRequests.delete(id);

                    // Send to content script
                    if (request.tabId) {
                        chrome.tabs.sendMessage(request.tabId, {
                            type: 'formats',
                            url: msg.url,
                            formats: msg.formats
                        });
                    }
                    break;
                }
            }
            break;

        case 'error':
            console.error('[tur] Native error:', msg.message);
            break;

        case 'download_started':
            console.log('[tur] Download started:', msg.id);
            break;
    }
}

function requestFormats(url, tabId) {
    const port = connectNativeHost();

    if (!port) {
        // Native host not available - fallback to deep link
        console.log('[tur] Native host not available, using deep link');
        sendToTur(url);
        return Promise.reject(new Error('Native host not available'));
    }

    return new Promise((resolve, reject) => {
        const requestId = Date.now().toString();
        pendingRequests.set(requestId, { resolve, reject, url, tabId });

        port.postMessage({ action: 'get_formats', url });

        // Timeout after 30 seconds
        setTimeout(() => {
            if (pendingRequests.has(requestId)) {
                pendingRequests.delete(requestId);
                reject(new Error('Timeout'));
            }
        }, 30000);
    });
}

// ============================================================================
// Context Menu
// ============================================================================

chrome.contextMenus.onClicked.addListener((info, tab) => {
    const url = info.linkUrl || info.srcUrl || info.pageUrl;
    if (url) {
        // Direct download for files, format fetch for video pages
        if (isDirectFileUrl(url)) {
            sendToTur(url);
        } else {
            requestFormats(url, tab?.id).catch(() => {
                // Fallback to deep link if native fails
                sendToTur(url);
            });
        }
    }
});

function isDirectFileUrl(url) {
    const directExtensions = ['.mp4', '.webm', '.avi', '.mkv', '.mov',
        '.mp3', '.flac', '.wav', '.zip', '.rar', '.exe', '.dmg', '.pdf'];
    const lowerUrl = url.toLowerCase();
    return directExtensions.some(ext => lowerUrl.includes(ext));
}

// ============================================================================
// Messages from Content Script
// ============================================================================

chrome.runtime.onMessage.addListener((message, sender, sendResponse) => {
    if (message.type === 'get_formats') {
        requestFormats(message.url, sender.tab?.id)
            .then(formats => sendResponse({ success: true, formats }))
            .catch(err => sendResponse({ success: false, error: err.message }));
        return true; // Async response
    }

    if (message.type === 'download') {
        console.log('[tur] Download request:', message.url, 'format:', message.formatId);

        const port = connectNativeHost();
        if (port) {
            port.postMessage({
                action: 'download',
                url: message.url,
                format_id: message.formatId
            });
        } else {
            sendToTur(message.url, message.quality);
        }

        sendResponse({ success: true });
    }

    return false;
});

// ============================================================================
// Deep Link Fallback
// ============================================================================

function sendToTur(url, quality) {
    const params = new URLSearchParams();
    params.set('url', url);
    if (quality) {
        params.set('quality', quality);
    }

    const turUrl = `${TUR_PROTOCOL}download?${params.toString()}`;
    console.log('[tur] Opening deep link:', turUrl.substring(0, 80));

    chrome.tabs.create({ url: turUrl, active: false }, (tab) => {
        setTimeout(() => {
            if (tab?.id) {
                chrome.tabs.remove(tab.id).catch(() => { });
            }
        }, 1500);
    });
}

console.log('[tur] Background service worker loaded');
