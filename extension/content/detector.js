window.Tur.detector = {
    detectVideos: function () {
        const state = window.Tur.state;
        const detector = window.Tur.detector;

        // Clear stale preload if URL changed (SPA navigation)
        if (state.lastUrl !== window.location.href) {
            state.lastUrl = window.location.href;
            state.preloadedFormats = null;
        }

        // Find videos in regular DOM
        const videos = document.querySelectorAll('video');
        videos.forEach(detector.processVideo);

        // Find videos inside Shadow DOM
        detector.findVideosInShadowDOM(document.body);
    },

    findVideosInShadowDOM: function (root) {
        const detector = window.Tur.detector;
        const elements = root.querySelectorAll('*');

        elements.forEach((el) => {
            if (el.shadowRoot) {
                const shadowVideos = el.shadowRoot.querySelectorAll('video');
                shadowVideos.forEach(detector.processVideo);
                detector.findVideosInShadowDOM(el.shadowRoot);
            }
        });
    },

    processVideo: function (video) {
        const state = window.Tur.state;
        const ui = window.Tur.ui;
        const messaging = window.Tur.messaging;
        const utils = window.Tur.utils;

        if (state.videoButtons.has(video)) return;

        utils.log('[tur] Found video:',
            'dims:', video.offsetWidth + 'x' + video.offsetHeight,
            'src:', (video.src || video.currentSrc || 'blob')?.slice(0, 50));

        if (video.offsetWidth < 200 || video.offsetHeight < 120) return;

        ui.createButtonForVideo(video);

        // Preload formats
        messaging.prefetchFormats();
    }
};
