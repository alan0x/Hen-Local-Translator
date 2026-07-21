import './voice-lab.css';
import VoiceLab from './VoiceLab.svelte';
import { mount } from 'svelte';

mount(VoiceLab, { target: document.getElementById('app')! });
