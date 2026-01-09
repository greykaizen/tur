window.Tur.messaging = {
    prefetchFormats: function () {
        const state = window.Tur.state;
        const utils = window.Tur.utils;

        if (state.preloadedFormats) {
            utils.log('[tur] Prefetch skipped - already cached');
            return;
        }

        const videoUrl = window.location.href;
        utils.log('[tur] Prefetching formats for:', videoUrl.slice(0, 50));

        try {
            chrome.runtime.sendMessage(
                { type: 'get_formats', url: videoUrl },
                (response) => {
                    utils.log('[tur] Prefetch response:', response);
                    if (response?.success) {
                        state.preloadedFormats = response;
                        utils.log('[tur] Formats cached:',
                            'videos:', response.videos?.length || 0,
                            'audios:', response.audios?.length || 0,
                            'subs:', response.subtitles?.length || 0);
                    } else {
                        utils.log('[tur] Prefetch failed:', response?.error);
                    }
                }
            );
        } catch (e) {
            utils.warn('[tur] Prefetch exception:', e.message);
        }
    },

    sendToTur: function (url, formatId, audioId, subLang, title, filesize, ext, videoStreamUrl, audioStreamUrl) {
        const utils = window.Tur.utils;
        utils.log('[tur] Sending to app:', url, 'format:', formatId, 'title:', title, 'size:', filesize, 'ext:', ext, 'streamUrl:', videoStreamUrl);

        utils.showToast('Requesting download...', 'info');

        try {
            chrome.runtime.sendMessage({
                type: 'download',
                url: url,
                formatId: formatId,
                audioId: audioId,
                subLang: subLang,
                title: title || '',
                filesize: filesize ? parseInt(filesize, 10) : null,
                ext: ext || '',
                videoStreamUrl: videoStreamUrl || '',
                audioStreamUrl: audioStreamUrl || ''
            }, (response) => {
                if (chrome.runtime.lastError) {
                    utils.warn('[tur] Runtime error:', chrome.runtime.lastError);
                    utils.showToast('Extension Error: Refresh page', 'error');
                    return;
                }

                if (response) {
                    if (response.success) {
                        utils.showToast('Download started in tur', 'info');
                    } else {
                        if (response.appNotRunning) {
                            utils.showToast('App Disconnected. Check extension icon.', 'error');
                        } else {
                            utils.showToast('Download failed: ' + (response.error || 'Unknown error'), 'error');
                        }
                    }
                }
            });
        } catch (e) {
            // Extension context invalidated - usually means extension was reloaded
            utils.warn('[tur] Extension context invalidated, please refresh the page');
            utils.showToast('Connection lost. Please refresh.', 'error');
        }
    }
};
