import { log, warn, error } from './utils.js';
import { cacheFormats } from './cache.js';

const TUR_HOST_NAME = 'com.greykaizen.tur';
let nativePort = null;
let isAppRunning = false;
const pendingRequests = new Map();
const uiPorts = new Set();

export function getAppStatus() {
    return { isAppRunning: isAppRunning && nativePort !== null };
}

export function addUiPort(port) {
    log('[tur] UI Connected:', port.sender?.tab?.id);
    uiPorts.add(port);
    port.onDisconnect.addListener(() => {
        log('[tur] UI Disconnected:', port.sender?.tab?.id);
        uiPorts.delete(port);
    });
}

function handleNativeMessage(msg) {
    switch (msg.type) {
        case 'pong':
            log('[tur] Connected to tur app v' + msg.version);
            isAppRunning = true;
            break;

        case 'formats':
            const formatsData = {
                videos: msg.videos || [],
                audios: msg.audios || [],
                subtitles: msg.subtitles || []
            };

            for (const [id, request] of pendingRequests) {
                if (request.url === msg.url) {
                    if (request.tabId) {
                        cacheFormats(request.tabId, msg.url, formatsData);
                    }
                    request.resolve(formatsData);
                    pendingRequests.delete(id);

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
            error('[tur] Native error:', msg.message);
            for (const [id, request] of pendingRequests) {
                request.reject(new Error(msg.message));
                pendingRequests.delete(id);
            }
            break;

        case 'download_started':
            log('[tur] Download started:', msg.id);
            break;
    }
}

export function connectNativeHost() {
    if (nativePort) {
        return nativePort;
    }

    log('[tur] Connecting to native host:', TUR_HOST_NAME);

    try {
        nativePort = chrome.runtime.connectNative(TUR_HOST_NAME);

        nativePort.onMessage.addListener((msg) => {
            log('[tur] Native message:', msg);
            handleNativeMessage(msg);
        });

        nativePort.onDisconnect.addListener(() => {
            const lastError = chrome.runtime.lastError;
            log('[tur] Native host disconnected:', lastError?.message);
            nativePort = null;
            isAppRunning = false;

            for (const [id, { reject }] of pendingRequests) {
                reject(new Error('Native host disconnected'));
            }
            pendingRequests.clear();

            log('[tur] Notifying', uiPorts.size, 'UI ports of disconnect');
            for (const port of uiPorts) {
                try {
                    port.postMessage({ type: 'app_disconnected' });
                } catch (e) {
                    warn('[tur] Port msg failed', e);
                }
            }
        });

        nativePort.postMessage({ action: 'ping' });
        return nativePort;

    } catch (e) {
        error('[tur] Failed to connect:', e);
        nativePort = null;
        return null;
    }
}

export async function requestFormats(url, tabId) {
    // Note: Caller handles cache check if desired, but native.js is raw request logic?
    // In background.js, requestFormats checked cache first.
    // I should probably inject cache check logic here or in main.js?
    // I'll leave cache check to main.js or move it here.
    // Moving it here creates circular dependency? (native -> cache ... cache -> ?)
    // No, cache doesn't depend on native.
    // But requestFormats is "logic".
    // I'll implement ONLY native logic here. "startFormatRequest".
    // But background.js `requestFormats` was the unified entry.
    // I will rename `requestFormats` to `fetchFormatsFromNative`.

    if (!nativePort || !isAppRunning) {
        log('[tur] No active connection (app not running)');
        return Promise.reject(new Error('App not running'));
    }

    return new Promise((resolve, reject) => {
        const requestId = Date.now().toString();
        pendingRequests.set(requestId, { resolve, reject, url, tabId });

        nativePort.postMessage({ action: 'get_formats', url });

        setTimeout(() => {
            if (pendingRequests.has(requestId)) {
                pendingRequests.delete(requestId);
                reject(new Error('Timeout'));
            }
        }, 30000);
    });
}

export function sendDownloadRequest(message, sendResponse) {
    if (nativePort && isAppRunning) {
        nativePort.postMessage({
            action: 'download',
            url: message.url,
            format_id: message.formatId || null,
            title: message.title || null,
            filesize: message.filesize || null,
            ext: message.ext || null,
            video_stream_url: message.videoStreamUrl || null,
            audio_stream_url: message.audioStreamUrl || null
        });
        sendResponse({ success: true });
    } else {
        log('[tur] App not running');
        sendResponse({ success: false, error: 'App not running', appNotRunning: true });
    }
}
