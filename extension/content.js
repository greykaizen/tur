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
    const videos = document.querySelectorAll('video');

    videos.forEach((video) => {
        if (videoButtons.has(video)) return;
        if (video.offsetWidth < 200 || video.offsetHeight < 120) return;

        createButtonForVideo(video);
    });
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

    setTimeout(() => {
        if (activeDropdown === dropdown) {
            showFormats(dropdown, videoUrl);
        }
    }, 1500);
}

function closeDropdown() {
    if (activeDropdown) {
        activeDropdown.remove();
        activeDropdown = null;
    }
}

function showFormats(dropdown, videoUrl) {
    const formats = [
        { quality: '1080p', type: 'MP4' },
        { quality: '720p', type: 'MP4' },
        { quality: '480p', type: 'MP4' },
        { quality: '360p', type: 'MP4' },
    ];

    dropdown.innerHTML = `
    <div class="tur-dropdown-header">Download Options</div>
    <div class="tur-dropdown-list">
      ${formats.map(f => `
        <button class="tur-format-btn" data-quality="${f.quality}">
          <span class="tur-format-quality">${f.quality}</span>
          <span class="tur-format-type">${f.type}</span>
        </button>
      `).join('')}
    </div>
    <div class="tur-dropdown-footer">
      <button class="tur-send-btn">Open in tur</button>
    </div>
  `;

    dropdown.querySelectorAll('.tur-format-btn').forEach((btn) => {
        btn.addEventListener('click', () => {
            sendToTur(videoUrl, btn.dataset.quality);
            closeDropdown();
        });
    });

    dropdown.querySelector('.tur-send-btn').addEventListener('click', () => {
        sendToTur(videoUrl);
        closeDropdown();
    });
}

// ============================================================================
// Helpers
// ============================================================================

function sendToTur(url, quality) {
    console.log('[tur] Sending to app:', url, 'quality:', quality);
    chrome.runtime.sendMessage({
        type: 'download',
        url: url,
        quality: quality
    });
}

// ============================================================================
// Initialize
// ============================================================================

if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', init);
} else {
    init();
}
