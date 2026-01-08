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
const CACHE_PREFIX = 'fmt_'; // Prefix for format cache keys
const CACHE_SIZE_LIMIT = 5 * 1024 * 1024; // 5MB limit

let nativePort = null;
const pendingRequests = new Map();

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

// NOTE: Per-tab cleanup commented out - 5MB limit with auto-cleanup handles this
// chrome.tabs.onRemoved.addListener(async (tabId) => {
//     try {
//         const all = await chrome.storage.session.get(null);
//         const keysToRemove = Object.keys(all).filter(k => k.startsWith(`${CACHE_PREFIX}${tabId}:`));
//         if (keysToRemove.length) {
//             await chrome.storage.session.remove(keysToRemove);
//             console.log('[tur] Cleared cache for closed tab:', tabId, keysToRemove.length, 'entries');
//         }
//     } catch (e) {
//         console.warn('[tur] Failed to clear tab cache:', e);
//     }
// });

// ============================================================================
// Format Cache (chrome.storage.session)
// ============================================================================

function getCacheKey(tabId, url) {
    return `${CACHE_PREFIX}${tabId}:${url}`;
}

async function getCachedFormats(tabId, url) {
    try {
        const key = getCacheKey(tabId, url);
        const result = await chrome.storage.session.get(key);
        if (result[key]) {
            console.log('[tur] Cache hit for tab', tabId);
            return result[key];
        }
    } catch (e) {
        console.warn('[tur] Cache read error:', e);
    }
    return null;
}

async function cacheFormats(tabId, url, formats) {
    try {
        const key = getCacheKey(tabId, url);

        // Check size and clean if needed
        const bytesUsed = await chrome.storage.session.getBytesInUse();
        if (bytesUsed > CACHE_SIZE_LIMIT) {
            console.log('[tur] Cache limit reached, cleaning oldest entries...');
            const all = await chrome.storage.session.get(null);
            const cacheKeys = Object.keys(all).filter(k => k.startsWith(CACHE_PREFIX));
            // Remove oldest 50%
            const toRemove = cacheKeys.slice(0, Math.ceil(cacheKeys.length / 2));
            await chrome.storage.session.remove(toRemove);
            console.log('[tur] Cleaned', toRemove.length, 'old cache entries');
        }

        await chrome.storage.session.set({ [key]: formats });
        console.log('[tur] Cached formats for tab', tabId);
    } catch (e) {
        console.warn('[tur] Cache write error:', e);
    }
}

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
            // Build formats object with videos, audios, subtitles
            const formatsData = {
                videos: msg.videos || [],
                audios: msg.audios || [],
                subtitles: msg.subtitles || []
            };

            // Find pending request, cache formats, and resolve
            for (const [id, request] of pendingRequests) {
                if (request.url === msg.url) {
                    // Cache with tabId for session-based caching
                    if (request.tabId) {
                        cacheFormats(request.tabId, msg.url, formatsData);
                    }

                    request.resolve(formatsData);
                    pendingRequests.delete(id);

                    // Send to content script
                    if (request.tabId) {
                        chrome.tabs.sendMessage(request.tabId, {
                            type: 'formats',
                            url: msg.url,
                            ...formatsData
                        }).catch(() => { });
                    }
                    break;
                }
            }
            break;

        case 'error':
            console.error('[tur] Native error:', msg.message);
            for (const [id, request] of pendingRequests) {
                request.reject(new Error(msg.message));
                pendingRequests.delete(id);
            }
            break;

        case 'download_started':
            console.log('[tur] Download started:', msg.id);
            break;
    }
}

async function requestFormats(url, tabId) {
    // Check cache first (per-tab)
    if (tabId) {
        const cached = await getCachedFormats(tabId, url);
        if (cached) {
            return cached;
        }
    }

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

        // Send to tur-host via native messaging (it will launch main app)
        const port = connectNativeHost();
        if (port) {
            port.postMessage({
                action: 'download',
                url: message.url,
                format_id: message.formatId || null
            });
            sendResponse({ success: true });
        } else {
            console.log('[tur] Native host not available');
            sendResponse({ success: false, error: 'Native host not available' });
        }
    }

    return false;
});

// ============================================================================
// Deep Link to Main App
// ============================================================================

function sendToTur(url, formatId) {
    const params = new URLSearchParams();
    params.set('url', url);
    if (formatId) {
        params.set('format', formatId);
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
