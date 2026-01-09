document.addEventListener('DOMContentLoaded', updateStatus);

function updateStatus() {
    const statusBox = document.getElementById('status-container');
    const statusText = document.getElementById('status-text');
    const actionBox = document.getElementById('action-container');

    statusBox.className = 'status-box'; // Reset
    statusText.textContent = 'Checking status...';

    chrome.runtime.sendMessage({ type: 'check_app_status' }, (response) => {
        if (response?.isAppRunning) {
            // Connected
            statusBox.classList.add('connected');
            statusText.textContent = 'App Connected';
            actionBox.innerHTML = `
                <div class="hint-text">
                    tur is running and ready.<br>
                    Go to a video page to download.
                </div>
            `;
        } else {
            // Disconnected
            statusBox.classList.add('disconnected');
            statusText.textContent = 'App Disconnected';
            actionBox.innerHTML = `
                <button id="connect-btn" class="btn btn-primary">Connect to App</button>
                <div class="hint-text">
                    Click to launch and connect to tur app.
                </div>
            `;

            document.getElementById('connect-btn').addEventListener('click', connectToApp);
        }
    });
}

function connectToApp() {
    const btn = document.getElementById('connect-btn');
    btn.textContent = 'Connecting...';
    btn.disabled = true;

    chrome.runtime.sendMessage({ type: 'connect_to_app' }, (response) => {
        if (response?.connected) {
            updateStatus();
        } else {
            btn.textContent = 'Failed - Try Again';
            btn.disabled = false;
        }
    });
}

// Settings
const DEFAULT_DESIGN = 'v1';

// Initialize
chrome.storage.sync.get(['tur_design'], (result) => {
    const design = result.tur_design || DEFAULT_DESIGN;
    const radio = document.querySelector(`input[name="design"][value="${design}"]`);
    if (radio) radio.checked = true;
});

// Save on change
document.querySelectorAll('input[name="design"]').forEach(radio => {
    radio.addEventListener('change', (e) => {
        if (e.target.checked) {
            chrome.storage.sync.set({ tur_design: e.target.value });
        }
    });
});
