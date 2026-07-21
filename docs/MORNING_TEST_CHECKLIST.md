# Morning Test Checklist

Test the new build in this order. Write down the step number and what happened
if anything is wrong.

## 1. DMG and blank-window regression

1. Quit every older Hen Local / Moxin Translator process first.
2. Open `dist/Hen-Local-Translator-v1.2.0-beta.1.dmg`.
3. Drag **Hen Local Translator** to Applications and launch that copy.
4. Confirm the control window has buttons/content instead of a blank white page.
5. Confirm the subtitle window also has content instead of a blank white page.
6. Switch Chinese/English and confirm the native window title changes between
   **很 Local 实时翻译** and **Hen Local Live Translator**.

## 2. First-run model delivery

1. On a Mac without the model folder, confirm first launch asks for the roughly
   4.2 GB core download.
2. Start, interrupt, and retry the download; confirm it resumes and eventually
   reports installed.
3. Confirm the app cannot start translation before core models are complete.
4. Turn on **译文播报** and confirm it works immediately with the voices built
   into macOS and does not ask for a separate 3.1 GB speech-model download.

## 3. Normal translation

1. Test microphone and system audio separately.
2. Test Chinese → English and English → Chinese.
3. Start and stop twice. Confirm the timer only moves while translating.
4. Confirm subtitles render in both floating and full-screen modes.
5. Confirm transcript export/settings remain unchanged.
6. Force-quit once while translation is running, reopen, and start again.
   Confirm it does not remain on **正在启动准备** and does not show
   `Bridge already connected` or `Failed to connect to Dora`.

## 4. Apple voice previews and live speech

0. Confirm **译文播报** is off on a fresh/default preference file. Open
   Settings → **Apple 音色实验室**, filter Chinese and English,试听候选音色,
   star favorites, and copy the shortlist.
1. For an English target, test exactly **Siri Voice 1–5** downloaded from
   macOS Live Speech settings. For a Chinese target, test **Apple 音色 1–5** and
   use the Voice Lab to shortlist better Chinese voices.
2. Confirm the first Chinese choice is **Yue (Premium)** and compare its startup
   speed and naturalness with the other Chinese candidates.
3. Confirm every preview speaks the new Hen Local sample, not “这是一个充满
   希望的时代” and not “欢迎来到 Moxie Voice”.
4. Start translation with speech enabled and confirm every completed translation
   is spoken once, in order.
5. Stop while a sentence is playing and confirm speech stops without leaving a
   `say` or Qwen TTS process behind.
6. Confirm output follows the Mac system-default device.
7. Confirm the app never asks to download a speech model and no Qwen TTS node
   appears in the running Dora dataflow.


## 5. Theme, logo, and settings

1. Test blue `#0003FE`, orange `#FF5705`, pink `#FF0073`, and green `#51F91B`.
2. Confirm the logo, underline, highlight, and main action use the chosen color.
3. Confirm orange/pink main-action text is white and green is readable.
4. Confirm the bottom subtitle-window logo remains black with a white mark.
5. Confirm the Dock/Menu Bar icon size looks normal and its theme color updates.
6. Confirm **转录记录** and the square black settings button have equal height.

## 6. Local usage and value

1. Open Settings and find **本机使用统计 / LOCAL USAGE**.
2. Confirm current session, this month, lifetime, and completed sessions make
   sense after starting/stopping a short translation.
3. Change the cloud comparison rate and confirm the estimated value updates.
4. Quit/reopen and confirm totals survive.
5. Confirm the wording says local-only, not used for billing, and estimate only.

## 7. Update shell

1. Open Settings → Check for Updates.
2. Confirm the current version is `1.2.0-beta.1` and no blank/error page appears.
3. A real upgrade, tamper rejection, and interrupted-download test require a new
   tagged build, so do not mark those passed from this same-version build.

## 8. Account and two-device license — after service configuration

The current internal build deliberately shows that the account service is not
configured and leaves local translation unlocked. After the protected Supabase,
Stripe, and login settings are supplied, test all of these:

1. Click **登录 / 创建账户** and confirm login completes in the browser and
   returns to the installed app through `henlocal://`.
2. Confirm the seven-day trial begins at first Mac activation, not account
   creation.
3. Confirm the device private key and session exist in macOS Keychain, while no
   server billing/signing secret appears in the app bundle.
4. Activate two Macs. Confirm both names and last-used dates appear.
5. Attempt a third Mac. Confirm it shows the two active Macs, then allows one to
   be deactivated before retrying.
6. Deactivate the current Mac. Confirm it signs out and does not silently
   reactivate itself.
7. Disconnect the network and confirm a signed license works only through its
   displayed date, never longer than seven days.
8. Move the Mac clock backward and confirm it does not extend access.
9. Subscribe for $49/month, open the billing portal, cancel, and confirm access
   continues through the paid-through date.
10. Fail a renewal and confirm the payment grace ends after three days instead
    of extending every time the daily reconciliation runs.
11. After expiry, confirm Settings and existing transcript export still work,
    while starting a new translation shows a direct recovery message.

## External items not ready to certify

Account and licensing code now exists on both the server and desktop, but the
real customer journey cannot be certified until it is connected to deployed
Supabase/Stripe/login environments. Apple Gatekeeper/notarization also awaits
Developer Program credentials. These are external setup blockers, not items to
pretend passed locally.
