import './styles.css';
import {initApp} from './app';
import {reconnectPeers} from './api/commands';

document.addEventListener('DOMContentLoaded', async () => {
    const root = document.getElementById('app');
    if (root) {
        await initApp(root);
    }

    // Handle app visibility changes (especially important for mobile)
    // When app returns from background, reconnect to peers
    document.addEventListener('visibilitychange', async () => {
        if (document.visibilityState === 'visible') {
            console.log('App became visible, reconnecting to peers...');
            try {
                await reconnectPeers();
            } catch (e) {
                console.error('Failed to reconnect peers:', e);
            }
        }
    });
});
