/**
 * tur Browser Extension - Content Script
 * 
 * Detects HTML5 <video> elements and shows download button
 * Button appears OUTSIDE video frame (above, aligned right)
 * Whole button is draggable
 * 
 * @author greykaizen
 */

// ============================================================================
// State
// ============================================================================

const videoButtons = new WeakMap();
let activeDropdown = null;
let preloadedFormats = null;
let lastUrl = window.location.href; // Track URL for SPA navigation

// ============================================================================
// Video Detection with MutationObserver
// ============================================================================

function init() {
    console.log('[tur] Content script loaded');

    detectVideos();

    const observer = new MutationObserver((mutations) => {
        for (const mutation of mutations) {
            if (mutation.addedNodes.length) {
                detectVideos();
            }
        }
    });

    observer.observe(document.body, {
        childList: true,
        subtree: true
    });

    document.addEventListener('fullscreenchange', handleFullscreenChange);
    document.addEventListener('webkitfullscreenchange', handleFullscreenChange);

    document.addEventListener('click', (e) => {
        if (activeDropdown && !activeDropdown.contains(e.target)) {
            const container = activeDropdown.closest('.tur-container');
            if (!container || !container.contains(e.target)) {
                closeDropdown();
            }
        }
    });

    document.addEventListener('keydown', (e) => {
        if (e.key === 'Escape' && activeDropdown) {
            closeDropdown();
        }
    });
}

function detectVideos() {
    // Clear stale preload if URL changed (SPA navigation)
    if (lastUrl !== window.location.href) {
        lastUrl = window.location.href;
        preloadedFormats = null;
    }

    // Find videos in regular DOM
    const videos = document.querySelectorAll('video');
    videos.forEach(processVideo);

    // Find videos inside Shadow DOM (Reddit uses this)
    findVideosInShadowDOM(document.body);
}

/**
 * Recursively search for videos inside Shadow DOM elements
 * Reddit's video player uses Shadow DOM, so we need to traverse into it
 */
function findVideosInShadowDOM(root) {
    const elements = root.querySelectorAll('*');

    elements.forEach((el) => {
        if (el.shadowRoot) {
            // Found a shadow root - search for videos inside
            const shadowVideos = el.shadowRoot.querySelectorAll('video');
            shadowVideos.forEach(processVideo);

            // Recursively search deeper shadow roots
            findVideosInShadowDOM(el.shadowRoot);
        }
    });
}

/**
 * Process a single video element
 */
function processVideo(video) {
    if (videoButtons.has(video)) return;

    console.log('[tur] Found video:',
        'dims:', video.offsetWidth + 'x' + video.offsetHeight,
        'src:', (video.src || video.currentSrc || 'blob')?.slice(0, 50));

    if (video.offsetWidth < 200 || video.offsetHeight < 120) return;

    createButtonForVideo(video);

    // Preload formats in background (like IDM)
    prefetchFormats();
}

/**
 * Prefetch formats for current page URL
 */
function prefetchFormats() {
    if (preloadedFormats) {
        console.log('[tur] Prefetch skipped - already cached');
        return;
    }

    const videoUrl = window.location.href;
    console.log('[tur] Prefetching formats for:', videoUrl.slice(0, 50));

    try {
        chrome.runtime.sendMessage(
            { type: 'get_formats', url: videoUrl },
            (response) => {
                console.log('[tur] Prefetch response:', response);
                if (response?.success) {
                    preloadedFormats = response;
                    console.log('[tur] Formats cached:',
                        'videos:', response.videos?.length || 0,
                        'audios:', response.audios?.length || 0,
                        'subs:', response.subtitles?.length || 0);
                } else {
                    console.log('[tur] Prefetch failed:', response?.error);
                }
            }
        );
    } catch (e) {
        console.warn('[tur] Prefetch exception:', e.message);
    }
}

// ============================================================================
// Button Creation
// ============================================================================

function createButtonForVideo(video) {
    const container = document.createElement('div');
    container.className = 'tur-container';

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
        videoButtons.delete(video);
    });

    container.appendChild(button);
    container.appendChild(closeBtn);

    document.body.appendChild(container);
    positionContainer(container, video);

    videoButtons.set(video, container);

    // Button click → show dropdown (only if not dragging)
    let wasDragging = false;
    button.addEventListener('click', (e) => {
        if (wasDragging) {
            wasDragging = false;
            return;
        }
        e.stopPropagation();
        toggleDropdown(container, video);
    });

    // Whole container is draggable
    makeDraggable(container, video, () => { wasDragging = true; });

    const reposition = () => {
        if (!container.dataset.manualPosition) {
            positionContainer(container, video);
        }
    };
    window.addEventListener('scroll', reposition, { passive: true });
    window.addEventListener('resize', reposition, { passive: true });

    const resizeObserver = new ResizeObserver(reposition);
    resizeObserver.observe(video);

    // Remove button when video goes off-screen (even if manually positioned)
    const visibilityObserver = new IntersectionObserver((entries) => {
        entries.forEach((entry) => {
            if (!entry.isIntersecting) {
                // Video is off-screen - remove button
                container.remove();
                videoButtons.delete(video);
                visibilityObserver.disconnect();
                resizeObserver.disconnect();
            }
        });
    }, { threshold: 0 });
    visibilityObserver.observe(video);
}

function positionContainer(container, video) {
    const rect = video.getBoundingClientRect();
    const containerWidth = container.offsetWidth || 160;

    container.style.position = 'fixed';
    container.style.top = `${rect.top - container.offsetHeight}px`;
    container.style.left = `${rect.right - containerWidth}px`;
    container.style.zIndex = '2147483646';
}

// ============================================================================
// Draggable (whole container)
// ============================================================================

function makeDraggable(container, video, onDragStart) {
    let isDragging = false;
    let hasMoved = false;
    let startX, startY, startLeft, startTop;

    container.addEventListener('mousedown', (e) => {
        // Allow close button click
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

        // Only count as drag if moved more than 3px
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
}

// ============================================================================
// Fullscreen Handling
// ============================================================================

function handleFullscreenChange() {
    const isFullscreen = !!(document.fullscreenElement || document.webkitFullscreenElement);

    document.querySelectorAll('.tur-container').forEach((container) => {
        container.style.display = isFullscreen ? 'none' : 'flex';
    });

    if (isFullscreen && activeDropdown) {
        closeDropdown();
    }
}

// ============================================================================
// Dropdown Menu
// ============================================================================

function toggleDropdown(container, video) {
    if (activeDropdown && activeDropdown.parentElement === container) {
        closeDropdown();
        return;
    }

    closeDropdown();

    const dropdown = document.createElement('div');
    dropdown.className = 'tur-dropdown';

    container.appendChild(dropdown);
    activeDropdown = dropdown;
    const videoUrl = window.location.href;

    // Use preloaded formats if available (instant display!)
    if (preloadedFormats?.success) {
        showFormats(dropdown, videoUrl, preloadedFormats);
        return;
    }

    // Show loading and fetch
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
                if (activeDropdown !== dropdown) return;
                if (response?.success) {
                    preloadedFormats = response; // Cache for next time
                    showFormats(dropdown, videoUrl, response);
                } else {
                    showError(dropdown, response?.error || 'Failed to fetch formats');
                }
            }
        );
    } catch (e) {
        console.warn('[tur] Extension context invalidated');
        showError(dropdown, 'Extension error - please refresh');
    }
}

function closeDropdown() {
    if (activeDropdown) {
        activeDropdown.remove();
        activeDropdown = null;
    }
}

function showFormats(dropdown, videoUrl, data) {
    console.log('[tur] showFormats data:', data);

    // Handle nested structure: { success, formats: { videos, audios, subtitles } }
    // or flat structure: { videos, audios, subtitles }
    const formats = data.formats || data;
    const videos = formats.videos || [];
    const audios = formats.audios || [];
    const subtitles = formats.subtitles || [];
    const browserLang = navigator.language.split('-')[0];

    console.log('[tur] Parsed:', videos.length, 'videos,', audios.length, 'audios,', subtitles.length, 'subs');

    dropdown.innerHTML = `
    <div class="tur-dropdown-header">Download Options</div>
    <div class="tur-tabs">
      <button class="tur-tab active" data-tab="video">Video (${videos.length})</button>
      <button class="tur-tab" data-tab="audio">Audio (${audios.length})</button>
      <button class="tur-tab" data-tab="subs">Subs (${subtitles.length})</button>
    </div>
    <div class="tur-tab-content" data-content="video">
      ${videos.length ? videos.map((f, i) => `
        <button class="tur-format-btn ${i === 0 ? 'selected' : ''}" data-format-id="${f.format_id}" data-label="${f.quality}" data-type="video">
          ${i === 0 ? '<span class="tur-best">Best</span>' : ''}
          <span class="tur-format-quality">${f.quality}</span>
          <span class="tur-format-type">${f.ext.toUpperCase()}${f.has_audio ? '' : ' (no audio)'}</span>
          ${f.filesize ? `<span class="tur-format-size">${formatSize(f.filesize)}</span>` : ''}
        </button>
      `).join('') : '<div class="tur-empty">No video formats</div>'}
    </div>
    <div class="tur-tab-content" data-content="audio" style="display:none">
      ${audios.length ? audios.map((f, i) => `
        <button class="tur-format-btn ${i === 0 ? 'selected' : ''}" data-format-id="${f.format_id}" data-label="${f.quality}" data-type="audio">
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

    // Tab switching
    dropdown.querySelectorAll('.tur-tab').forEach(tab => {
        tab.addEventListener('click', () => {
            dropdown.querySelectorAll('.tur-tab').forEach(t => t.classList.remove('active'));
            tab.classList.add('active');
            dropdown.querySelectorAll('.tur-tab-content').forEach(c => c.style.display = 'none');
            dropdown.querySelector(`[data-content="${tab.dataset.tab}"]`).style.display = 'block';
        });
    });

    // Format selection
    dropdown.querySelectorAll('.tur-format-btn').forEach(btn => {
        btn.addEventListener('click', () => {
            const content = btn.closest('.tur-tab-content');
            content.querySelectorAll('.tur-format-btn').forEach(b => b.classList.remove('selected'));
            btn.classList.add('selected');
            updateSummary();
        });
    });

    // Download button
    dropdown.querySelector('.tur-send-btn').addEventListener('click', () => {
        const selectedVideo = dropdown.querySelector('[data-content="video"] .selected');
        const selectedAudio = dropdown.querySelector('[data-content="audio"] .selected');
        const selectedSub = dropdown.querySelector('[data-content="subs"] .selected');
        sendToTur(videoUrl, selectedVideo?.dataset.formatId || '', selectedAudio?.dataset.formatId || '', selectedSub?.dataset.lang || '');
        closeDropdown();
    });
}

function showError(dropdown, message) {
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
        sendToTur(window.location.href);
        closeDropdown();
    });
}

function formatSize(bytes) {
    if (bytes < 1024 * 1024) return (bytes / 1024).toFixed(1) + ' KB';
    return (bytes / (1024 * 1024)).toFixed(1) + ' MB';
}

// ============================================================================
// Helpers
// ============================================================================

function sendToTur(url, formatId, audioId, subLang) {
    console.log('[tur] Sending to app:', url, 'format:', formatId, 'audio:', audioId, 'sub:', subLang);
    try {
        chrome.runtime.sendMessage({
            type: 'download',
            url: url,
            formatId: formatId,
            audioId: audioId,
            subLang: subLang
        });
    } catch (e) {
        // Extension context invalidated - usually means extension was reloaded
        console.warn('[tur] Extension context invalidated, please refresh the page');
    }
}

// ============================================================================
// Initialize
// ============================================================================

if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', init);
} else {
    init();
}
