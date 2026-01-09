import { log } from './utils.js';
import { getCachedFormats, cacheFormats } from './cache.js';
import { connectNativeHost, requestFormats, sendDownloadRequest, getAppStatus, addUiPort } from './native.js';

// ============================================================================
// Initialization
// ============================================================================

chrome.runtime.onInstalled.addListener(() => {
    chrome.contextMenus.create({
        id: 'download-with-tur',
        title: 'Download with tur',
        contexts: ['link', 'video', 'audio', 'image']
    });
    log('[tur] Extension installed');
});

// ============================================================================
// Core Logic
// ============================================================================

async function fetchFormats(url, tabId) {
    // Check cache first (per-tab)
    if (tabId) {
        const cached = await getCachedFormats(tabId, url);
        if (cached) {
            return cached;
        }
    }

    // Check app status BEFORE calling requestFormats
    // requestFormats handles the timeout logic, but we can fast-fail here
    const { isAppRunning } = getAppStatus();
    // But checking nativePort is inside native.js.
    // requestFormats handles this check internally too.
    return requestFormats(url, tabId);
}

// ============================================================================
// Context Menu
// ============================================================================

chrome.contextMenus.onClicked.addListener((info, tab) => {
    const url = info.linkUrl || info.srcUrl || info.pageUrl;
    if (url) {
        // Check app status
        const { isAppRunning } = getAppStatus();
        if (!isAppRunning) {
            log('[tur] Context menu: app not running');
            return;
        }

        if (isDirectFileUrl(url)) {
            sendDownloadRequest({ url }, () => { }); // Fire and forget
        } else {
            fetchFormats(url, tab?.id).catch((err) => {
                log('[tur] Context menu format fetch failed:', err.message);
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
// Messages
// ============================================================================

chrome.runtime.onMessage.addListener((message, sender, sendResponse) => {
    if (message.type === 'get_formats') {
        fetchFormats(message.url, sender.tab?.id)
            .then(formats => sendResponse({ success: true, formats }))
            .catch(err => sendResponse({ success: false, error: err.message }));
        return true; // Async
    }

    if (message.type === 'download') {
        log('[tur] Download request:', message.url);
        sendDownloadRequest(message, sendResponse);
        return false; // Sync response from sendDownloadRequest is handled immediately?
        // Wait, sendDownloadRequest calls sendResponse immediately? Yes.
    }

    if (message.type === 'check_app_status') {
        const { isAppRunning } = getAppStatus();
        sendResponse({ isAppRunning });
        return false;
    }

    if (message.type === 'connect_to_app') {
        log('[tur] User requested connection to app');
        connectNativeHost();

        setTimeout(() => {
            const { isAppRunning } = getAppStatus();
            sendResponse({ connected: isAppRunning });
        }, 1000);
        return true; // Async
    }

    return false;
});

// ============================================================================
// UI Port Management
// ============================================================================

chrome.runtime.onConnect.addListener((port) => {
    if (port.name === 'tur-ui') {
        addUiPort(port);
    }
});

log('[tur] Background service worker loaded (Modular)');
