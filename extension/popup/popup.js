document.getElementById('open-app').addEventListener('click', () => {
    chrome.tabs.create({ url: 'tur://open' });
});
