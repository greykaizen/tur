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

    // Log video for debugging
    console.log('[tur] Found video:',
        'dims:', video.offsetWidth + 'x' + video.offsetHeight,
        'src:', (video.src || video.currentSrc || 'blob')?.slice(0, 50));

    if (video.offsetWidth < 200 || video.offsetHeight < 120) return;

    createButtonForVideo(video);
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
    dropdown.innerHTML = `
    <div class="tur-dropdown-header">Download Options</div>
    <div class="tur-dropdown-loading">
      <div class="tur-spinner"></div>
      <span>Fetching formats...</span>
    </div>
  `;

    container.appendChild(dropdown);
    activeDropdown = dropdown;

    const videoUrl = window.location.href;

    // Request formats from native host via background script
    try {
        chrome.runtime.sendMessage(
            { type: 'get_formats', url: videoUrl },
            (response) => {
                if (activeDropdown !== dropdown) return; // Dropdown was closed

                if (response?.success && response.formats) {
                    showFormats(dropdown, videoUrl, response.formats);
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

function showFormats(dropdown, videoUrl, formats) {
    dropdown.innerHTML = `
    <div class="tur-dropdown-header">Download Options</div>
    <div class="tur-dropdown-list">
      ${formats.map(f => `
        <button class="tur-format-btn" data-format-id="${f.format_id}">
          <span class="tur-format-quality">${f.quality}</span>
          <span class="tur-format-type">${f.ext.toUpperCase()}</span>
          ${f.filesize ? `<span class="tur-format-size">${formatSize(f.filesize)}</span>` : ''}
        </button>
      `).join('')}
    </div>
    <div class="tur-dropdown-footer">
      <button class="tur-send-btn">Open in tur</button>
    </div>
  `;

    dropdown.querySelectorAll('.tur-format-btn').forEach((btn) => {
        btn.addEventListener('click', () => {
            sendToTur(videoUrl, btn.dataset.formatId);
            closeDropdown();
        });
    });

    dropdown.querySelector('.tur-send-btn').addEventListener('click', () => {
        sendToTur(videoUrl);
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

function sendToTur(url, quality) {
    console.log('[tur] Sending to app:', url, 'quality:', quality);
    try {
        chrome.runtime.sendMessage({
            type: 'download',
            url: url,
            quality: quality
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
