/**
 * tur Browser Extension - Background Service Worker
 * 
 * Handles messages from content script and sends to tur app
 * 
 * @author greykaizen
 */

const TUR_PROTOCOL = 'tur://';

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
// Context Menu
// ============================================================================

chrome.contextMenus.onClicked.addListener((info, tab) => {
    const url = info.linkUrl || info.srcUrl || info.pageUrl;
    if (url) {
        sendToTur(url);
    }
});

// ============================================================================
// Messages from Content Script
// ============================================================================

chrome.runtime.onMessage.addListener((message, sender, sendResponse) => {
    if (message.type === 'download') {
        console.log('[tur] Download request:', message.url, 'quality:', message.quality);
        sendToTur(message.url, message.quality);
        sendResponse({ success: true });
    }
    return false;
});

// ============================================================================
// Send to tur App
// ============================================================================

function sendToTur(url, quality) {
    const params = new URLSearchParams();
    params.set('url', url);
    if (quality) {
        params.set('quality', quality);
    }

    const turUrl = `${TUR_PROTOCOL}download?${params.toString()}`;
    console.log('[tur] Opening deep link:', turUrl.substring(0, 80));

    // Open deep link in new tab, then close it
    chrome.tabs.create({ url: turUrl, active: false }, (tab) => {
        setTimeout(() => {
            if (tab?.id) {
                chrome.tabs.remove(tab.id).catch(() => { });
            }
        }, 1500);
    });
}

console.log('[tur] Background service worker loaded');
