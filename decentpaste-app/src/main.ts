import './styles.css';
import {initApp} from './app';

document.addEventListener('DOMContentLoaded', async () => {
    const root = document.getElementById('app');
    if (root) {
        await initApp(root);
    }
});
