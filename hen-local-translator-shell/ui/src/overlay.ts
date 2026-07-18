import './overlay.css';
import Overlay from './Overlay.svelte';
import { mount } from 'svelte';

mount(Overlay, { target: document.getElementById('app')! });
