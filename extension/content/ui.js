window.Tur.ui = {
    createButtonForVideo: function (video) {
        const state = window.Tur.state;
        const ui = window.Tur.ui; // Access self methods

        const container = document.createElement('div');
        // Initial class - waiting for storage
        container.className = 'tur-container';

        // Apply saved design
        chrome.storage.sync.get(['tur_design'], (result) => {
            if (result.tur_design) {
                container.classList.add(`tur-btn-${result.tur_design}`);
            } else {
                container.classList.add('tur-btn-v1');
            }
        });

        // Listen for changes
        chrome.storage.onChanged.addListener((changes) => {
            if (changes.tur_design) {
                container.classList.remove('tur-btn-v1', 'tur-btn-v2', 'tur-btn-v3', 'tur-btn-v4');
                container.classList.add(`tur-btn-${changes.tur_design.newValue}`);
            }
        });

        // Button with logo + text
        const button = document.createElement('button');
        button.className = 'tur-download-btn';
        button.innerHTML = `
        <img src="${chrome.runtime.getURL('icons/icon16.png')}" class="tur-logo" alt="">
        <span>Download with tur</span>
      `;

        // Close (X) button
        const closeBtn = document.createElement('button');
        closeBtn.className = 'tur-close-btn';
        closeBtn.innerHTML = '×';
        closeBtn.addEventListener('click', (e) => {
            e.stopPropagation();
            container.remove();
            state.videoButtons.delete(video);
        });

        container.appendChild(button);
        container.appendChild(closeBtn);

        document.body.appendChild(container);
        ui.positionContainer(container, video);

        state.videoButtons.set(video, container);

        // Button click → show dropdown (only if not dragging)
        let wasDragging = false;
        button.addEventListener('click', (e) => {
            if (wasDragging) {
                wasDragging = false;
                return;
            }
            e.stopPropagation();
            ui.toggleDropdown(container, video);
        });

        // Whole container is draggable
        ui.makeDraggable(container, video, () => { wasDragging = true; });

        const reposition = () => {
            if (!container.dataset.manualPosition) {
                ui.positionContainer(container, video);
            }
        };
        window.addEventListener('scroll', reposition, { passive: true });
        window.addEventListener('resize', reposition, { passive: true });

        const resizeObserver = new ResizeObserver(reposition);
        resizeObserver.observe(video);

        // Remove button when video goes off-screen
        const visibilityObserver = new IntersectionObserver((entries) => {
            entries.forEach((entry) => {
                if (!entry.isIntersecting) {
                    container.remove();
                    state.videoButtons.delete(video);
                    visibilityObserver.disconnect();
                    resizeObserver.disconnect();
                }
            });
        }, { threshold: 0 });
        visibilityObserver.observe(video);
    },

    positionContainer: function (container, video) {
        const rect = video.getBoundingClientRect();
        const containerWidth = container.offsetWidth || 160;

        container.style.position = 'fixed';
        container.style.top = `${rect.top - container.offsetHeight}px`;
        container.style.left = `${rect.right - containerWidth}px`;
        container.style.zIndex = '2147483646';
    },

    makeDraggable: function (container, video, onDragStart) {
        let isDragging = false;
        let hasMoved = false;
        let startX, startY, startLeft, startTop;

        container.addEventListener('mousedown', (e) => {
            if (e.target.closest('.tur-close-btn')) return;

            isDragging = true;
            hasMoved = false;
            startX = e.clientX;
            startY = e.clientY;
            startLeft = container.offsetLeft;
            startTop = container.offsetTop;

            container.style.cursor = 'grabbing';
        });

        document.addEventListener('mousemove', (e) => {
            if (!isDragging) return;

            const dx = e.clientX - startX;
            const dy = e.clientY - startY;

            if (Math.abs(dx) > 3 || Math.abs(dy) > 3) {
                hasMoved = true;
                onDragStart();
                container.style.left = `${startLeft + dx}px`;
                container.style.top = `${startTop + dy}px`;
                container.dataset.manualPosition = 'true';
            }
        });

        document.addEventListener('mouseup', () => {
            if (isDragging) {
                isDragging = false;
                container.style.cursor = '';
            }
        });
    },

    handleFullscreenChange: function () {
        const isFullscreen = !!(document.fullscreenElement || document.webkitFullscreenElement);

        document.querySelectorAll('.tur-container').forEach((container) => {
            container.style.display = isFullscreen ? 'none' : 'flex';
        });

        if (isFullscreen && window.Tur.state.activeDropdown) {
            window.Tur.ui.closeDropdown();
        }
    },

    toggleDropdown: function (container, video) {
        const state = window.Tur.state;
        const ui = window.Tur.ui;
        const utils = window.Tur.utils;

        if (state.activeDropdown && state.activeDropdown.parentElement === container) {
            ui.closeDropdown();
            return;
        }

        ui.closeDropdown();

        const dropdown = document.createElement('div');
        dropdown.className = 'tur-dropdown';

        container.appendChild(dropdown);
        state.activeDropdown = dropdown;
        const videoUrl = window.location.href;

        // Theme support
        if (window.matchMedia && !window.matchMedia('(prefers-color-scheme: dark)').matches) {
            dropdown.classList.add('tur-light');
        }

        // Optimistic UI
        if (state.preloadedFormats?.success) {
            ui.showFormats(dropdown, videoUrl, state.preloadedFormats);
        } else {
            dropdown.innerHTML = `
            <div class="tur-dropdown-header">Download Options</div>
            <div class="tur-dropdown-loading">
              <div class="tur-spinner"></div>
              <span>Checking app...</span>
            </div>
          `;
        }

        try {
            state.uiPort = chrome.runtime.connect({ name: 'tur-ui' });
            state.uiPort.onMessage.addListener((msg) => {
                if (msg.type === 'app_disconnected') {
                    utils.log('[tur] Received disconnect signal via port');
                    if (state.activeDropdown) {
                        ui.showConnectPrompt(state.activeDropdown, null, null, window.location.href);
                    }
                }
            });
        } catch (e) {
            utils.warn('[tur] Failed to connect UI port:', e);
        }

        chrome.runtime.sendMessage({ type: 'check_app_status' }, (statusResponse) => {
            if (state.activeDropdown !== dropdown) return;

            if (!statusResponse?.isAppRunning) {
                ui.showConnectPrompt(dropdown, container, video, videoUrl);
                return;
            }

            if (!state.preloadedFormats?.success) {
                ui.fetchFormats(dropdown, videoUrl);
            }
        });
    },

    showConnectPrompt: function (dropdown, container, video, videoUrl) {
        dropdown.innerHTML = `
        <div class="tur-dropdown-header">Download Options</div>
        <div class="tur-app-not-running">
            <div class="tur-warning-icon">⚠️</div>
            <div class="tur-warning-text">App Disconnected</div>
            <div class="tur-warning-hint">Connect to enable downloads</div>
            <button class="tur-launch-btn tur-connect-btn">Connect to App</button>
        </div>
        `;

        const connectBtn = dropdown.querySelector('.tur-connect-btn');
        if (connectBtn) {
            connectBtn.addEventListener('click', () => {
                const ui = window.Tur.ui;
                const utils = window.Tur.utils;
                const state = window.Tur.state;

                connectBtn.textContent = 'Connecting...';
                connectBtn.disabled = true;

                chrome.runtime.sendMessage({ type: 'connect_to_app' }, (connectResponse) => {
                    if (connectResponse?.connected) {
                        utils.showToast('Connected to tur app', 'info');
                        if (state.preloadedFormats?.success) {
                            ui.showFormats(dropdown, videoUrl, state.preloadedFormats);
                        } else {
                            ui.fetchFormats(dropdown, videoUrl);
                        }
                    } else {
                        connectBtn.textContent = 'App not found - Try Again';
                        connectBtn.disabled = false;
                        utils.showToast('Connection failed. Is app installed?', 'error');

                        setTimeout(() => {
                            if (state.activeDropdown === dropdown && connectBtn.isConnected) {
                                connectBtn.textContent = 'Connect to App';
                            }
                        }, 2000);
                    }
                });
            });
        }
    },

    fetchFormats: function (dropdown, videoUrl) {
        const state = window.Tur.state;
        const ui = window.Tur.ui;
        // const utils = window.Tur.utils;

        dropdown.innerHTML = `
        <div class="tur-dropdown-header">Download Options</div>
        <div class="tur-dropdown-loading">
          <div class="tur-spinner"></div>
          <span>Fetching formats...</span>
        </div>
      `;

        try {
            chrome.runtime.sendMessage(
                { type: 'get_formats', url: videoUrl },
                (response) => {
                    if (state.activeDropdown !== dropdown) return;

                    if (response?.success) {
                        state.preloadedFormats = response;
                        ui.showFormats(dropdown, videoUrl, response);
                    } else if (response?.error === 'App not running') {
                        ui.showConnectPrompt(dropdown, null, null, videoUrl);
                    } else {
                        ui.showError(dropdown, response?.error || 'Failed to fetch formats');
                    }
                }
            );
        } catch (e) {
            ui.showError(dropdown, 'Extension error - please refresh');
        }
    },

    closeDropdown: function () {
        const state = window.Tur.state;
        if (state.activeDropdown) {
            state.activeDropdown.remove();
            state.activeDropdown = null;
        }
        if (state.uiPort) {
            state.uiPort.disconnect();
            state.uiPort = null;
        }
    },

    showFormats: function (dropdown, videoUrl, data) {
        const utils = window.Tur.utils;
        const ui = window.Tur.ui;
        const state = window.Tur.state;

        // data.formats
        const formats = data.formats || data;
        const videos = formats.videos || [];
        const audios = formats.audios || [];
        const subtitles = formats.subtitles || [];
        const browserLang = navigator.language.split('-')[0];

        dropdown.innerHTML = `
        <div class="tur-dropdown-header">
            Download Options
            <span class="tur-refresh-btn" title="Refresh links" style="float:right;cursor:pointer;opacity:0.7">↻</span>
        </div>
        <div class="tur-tabs">
          <button class="tur-tab active" data-tab="video">Video (${videos.length})</button>
          <button class="tur-tab" data-tab="audio">Audio (${audios.length})</button>
          <button class="tur-tab" data-tab="subs">Subs (${subtitles.length})</button>
        </div>
        <div class="tur-tab-content" data-content="video">
          ${videos.length ? videos.map((f, i) => `
            <button class="tur-format-btn ${i === 0 ? 'selected' : ''}" data-format-id="${f.format_id}" data-label="${f.quality}" data-type="video" data-ext="${f.ext}" data-filesize="${f.filesize || ''}" data-url="${f.url || ''}">
              ${i === 0 ? '<span class="tur-best">Best</span>' : ''}
              <span class="tur-format-quality">${f.quality}</span>
              <span class="tur-format-type">${f.ext.toUpperCase()}${f.has_audio ? '' : ' (no audio)'}</span>
              ${f.filesize ? `<span class="tur-format-size">${utils.formatSize(f.filesize)}</span>` : ''}
            </button>
          `).join('') : '<div class="tur-empty">No video formats</div>'}
        </div>
        <div class="tur-tab-content" data-content="audio" style="display:none">
          ${audios.length ? audios.map((f, i) => `
            <button class="tur-format-btn ${i === 0 ? 'selected' : ''}" data-format-id="${f.format_id}" data-label="${f.quality}" data-type="audio" data-ext="${f.ext}" data-filesize="${f.filesize || ''}" data-url="${f.url || ''}">
              ${i === 0 ? '<span class="tur-best">Best</span>' : ''}
              <span class="tur-format-quality">${f.quality}</span>
              <span class="tur-format-type">${f.ext.toUpperCase()}</span>
              ${f.language ? `<span class="tur-format-lang">${f.language}</span>` : ''}
            </button>
          `).join('') : '<div class="tur-empty">No audio-only formats</div>'}
        </div>
        <div class="tur-tab-content" data-content="subs" style="display:none">
          ${subtitles.length ? subtitles.map((s, i) => `
            <button class="tur-format-btn ${s.lang === browserLang || (i === 0 && !subtitles.find(x => x.lang === browserLang)) ? 'selected' : ''}" data-lang="${s.lang}" data-label="${s.name}" data-type="sub">
              ${s.lang === browserLang ? '<span class="tur-best">Auto</span>' : ''}
              <span class="tur-format-quality">${s.name}</span>
              <span class="tur-format-type">${s.ext.toUpperCase()}</span>
            </button>
          `).join('') : '<div class="tur-empty">No subtitles</div>'}
        </div>
        <div class="tur-selection-summary"></div>
        <div class="tur-dropdown-footer">
          <button class="tur-send-btn">Download</button>
        </div>
        `;

        const updateSummary = () => {
            const v = dropdown.querySelector('[data-content="video"] .selected');
            const a = dropdown.querySelector('[data-content="audio"] .selected');
            const s = dropdown.querySelector('[data-content="subs"] .selected');
            const parts = [];
            if (v) parts.push(`V: ${v.dataset.label}`);
            if (a) parts.push(`A: ${a.dataset.label}`);
            if (s) parts.push(`S: ${s.dataset.label}`);
            dropdown.querySelector('.tur-selection-summary').textContent = parts.join(' • ') || 'No selection';
        };
        updateSummary();

        dropdown.querySelectorAll('.tur-tab').forEach(tab => {
            tab.addEventListener('click', () => {
                dropdown.querySelectorAll('.tur-tab').forEach(t => t.classList.remove('active'));
                tab.classList.add('active');
                dropdown.querySelectorAll('.tur-tab-content').forEach(c => c.style.display = 'none');
                dropdown.querySelector(`[data-content="${tab.dataset.tab}"]`).style.display = 'block';
            });
        });

        dropdown.querySelectorAll('.tur-format-btn').forEach(btn => {
            btn.addEventListener('click', () => {
                const content = btn.closest('.tur-tab-content');
                content.querySelectorAll('.tur-format-btn').forEach(b => b.classList.remove('selected'));
                btn.classList.add('selected');
                updateSummary();
            });
        });

        dropdown.querySelector('.tur-send-btn').addEventListener('click', () => {
            const selectedVideo = dropdown.querySelector('[data-content="video"] .selected');
            const selectedAudio = dropdown.querySelector('[data-content="audio"] .selected');
            const selectedSub = dropdown.querySelector('[data-content="subs"] .selected');

            // Get video title
            let videoTitle = document.querySelector('h1.ytd-video-primary-info-renderer yt-formatted-string, h1.ytd-watch-metadata yt-formatted-string, #title h1')?.textContent?.trim()
                || document.title.replace(' - YouTube', '').trim()
                || 'download';

            const ext = selectedVideo?.dataset.ext || selectedAudio?.dataset.ext || '';
            const filesize = selectedVideo?.dataset.filesize || selectedAudio?.dataset.filesize || '';

            if (ext && !videoTitle.endsWith('.' + ext)) {
                videoTitle += '.' + ext;
            }

            const videoStreamUrl = selectedVideo?.dataset.url || '';
            const audioStreamUrl = selectedAudio?.dataset.url || '';

            window.Tur.messaging.sendToTur(videoUrl, selectedVideo?.dataset.formatId || '', selectedAudio?.dataset.formatId || '', selectedSub?.dataset.lang || '', videoTitle, filesize, ext, videoStreamUrl, audioStreamUrl);
            ui.closeDropdown();
        });

        const refreshBtn = dropdown.querySelector('.tur-refresh-btn');
        if (refreshBtn) {
            refreshBtn.addEventListener('click', () => {
                refreshBtn.style.animation = 'tur-spin 1s infinite linear';
                state.preloadedFormats = null;
                ui.fetchFormats(dropdown, videoUrl);
            });
        }
    },

    showError: function (dropdown, message) {
        const messaging = window.Tur.messaging;
        const ui = window.Tur.ui;

        dropdown.innerHTML = `
        <div class="tur-dropdown-header">Download Options</div>
        <div class="tur-dropdown-error">
          <span>⚠️ ${message}</span>
        </div>
        <div class="tur-dropdown-footer">
          <button class="tur-send-btn">Open in tur app</button>
        </div>
        `;

        dropdown.querySelector('.tur-send-btn').addEventListener('click', () => {
            messaging.sendToTur(window.location.href);
            ui.closeDropdown();
        });
    }
};
