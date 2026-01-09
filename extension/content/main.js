(function () {
    function init() {
        // Dependencies are guaranteed to be loaded by manifest order
        const utils = window.Tur.utils;
        const detector = window.Tur.detector;
        const ui = window.Tur.ui;
        const state = window.Tur.state;

        utils.log('[tur] Content script loaded (Modular)');

        detector.detectVideos();

        const observer = new MutationObserver((mutations) => {
            for (const mutation of mutations) {
                if (mutation.addedNodes.length) {
                    detector.detectVideos();
                }
            }
        });

        observer.observe(document.body, {
            childList: true,
            subtree: true
        });

        document.addEventListener('fullscreenchange', ui.handleFullscreenChange);
        document.addEventListener('webkitfullscreenchange', ui.handleFullscreenChange);

        document.addEventListener('click', (e) => {
            if (state.activeDropdown && !state.activeDropdown.contains(e.target)) {
                const container = state.activeDropdown.closest('.tur-container');
                if (!container || !container.contains(e.target)) {
                    ui.closeDropdown();
                }
            }
        });

        document.addEventListener('keydown', (e) => {
            if (e.key === 'Escape' && state.activeDropdown) {
                ui.closeDropdown();
            }
        });
    }

    if (document.readyState === 'loading') {
        document.addEventListener('DOMContentLoaded', init);
    } else {
        init();
    }
})();
