window.Tur.utils = {
    log: (...args) => window.Tur.state.DEBUG && console.log(...args),
    warn: (...args) => window.Tur.state.DEBUG && console.warn(...args),
    error: (...args) => window.Tur.state.DEBUG && console.error(...args),

    formatSize: (bytes) => {
        if (bytes < 1024 * 1024) return (bytes / 1024).toFixed(1) + ' KB';
        return (bytes / (1024 * 1024)).toFixed(1) + ' MB';
    },

    showToast: (message, type = 'info') => {
        const toast = document.createElement('div');
        toast.className = `tur-toast ${type}`;
        // Theme support
        if (window.matchMedia && !window.matchMedia('(prefers-color-scheme: dark)').matches) {
            toast.classList.add('tur-light');
        }

        toast.textContent = message;
        document.body.appendChild(toast);

        setTimeout(() => {
            toast.style.animation = 'tur-slide-out 0.3s ease-in forwards';
            setTimeout(() => toast.remove(), 300);
        }, 4000);
    }
};
