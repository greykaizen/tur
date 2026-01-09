import { log, warn } from './utils.js';

const CACHE_PREFIX = 'fmt_';
const CACHE_SIZE_LIMIT = 5 * 1024 * 1024; // 5MB limit

function getCacheKey(tabId, url) {
    return `${CACHE_PREFIX}${tabId}:${url}`;
}

export async function getCachedFormats(tabId, url) {
    try {
        const key = getCacheKey(tabId, url);
        const result = await chrome.storage.session.get(key);
        if (result[key]) {
            log('[tur] Cache hit for tab', tabId);
            return result[key];
        }
    } catch (e) {
        warn('[tur] Cache read error:', e);
    }
    return null;
}

export async function cacheFormats(tabId, url, formats) {
    try {
        const key = getCacheKey(tabId, url);

        // Check size and clean if needed
        const bytesUsed = await chrome.storage.session.getBytesInUse();
        if (bytesUsed > CACHE_SIZE_LIMIT) {
            log('[tur] Cache limit reached, cleaning oldest entries...');
            const all = await chrome.storage.session.get(null);
            const cacheKeys = Object.keys(all).filter(k => k.startsWith(CACHE_PREFIX));
            // Remove oldest 50%
            const toRemove = cacheKeys.slice(0, Math.ceil(cacheKeys.length / 2));
            await chrome.storage.session.remove(toRemove);
            log('[tur] Cleaned', toRemove.length, 'old cache entries');
        }

        await chrome.storage.session.set({ [key]: formats });
        log('[tur] Cached formats for tab', tabId);
    } catch (e) {
        warn('[tur] Cache write error:', e);
    }
}
