const { app, BrowserWindow, ipcMain } = require('electron');
const path = require('path');
const fs = require('fs');

let win;
const args = process.argv.slice(2);
let outputFile = '/tmp/electron_term.txt';
let readyFile = '/tmp/electron_term.ready';

for (const a of args) {
    if (a.endsWith('.txt') || a.endsWith('.bin')) outputFile = a;
    if (a.endsWith('.ready')) readyFile = a;
}

app.whenReady().then(() => {
    win = new BrowserWindow({
        width: 1000,
        height: 600,
        title: 'clak_electron_xterm',
        webPreferences: {
            nodeIntegration: true,
            contextIsolation: false
        }
    });

    win.loadFile(path.join(__dirname, 'index.html'), {
        query: { out: outputFile }
    });

    ipcMain.on('ready', () => {
        fs.writeFileSync(readyFile, 'ready\n');
    });

    ipcMain.on('finish', () => {
        app.quit();
    });
});
