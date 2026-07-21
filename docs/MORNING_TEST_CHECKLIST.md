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
4. Turn on **译文播报** and confirm only then it asks for the optional roughly
   3.1 GB spoken-translation model.

## 3. Normal translation

1. Test microphone and system audio separately.
2. Test Chinese → English and English → Chinese.
3. Start and stop twice. Confirm the timer only moves while translating.
4. Confirm subtitles render in both floating and full-screen modes.
5. Confirm transcript export/settings remain unchanged.

## 4. Eight voice previews

Test exactly: Vivian, Serena, 白杨, 杨阳, Ryan, Aiden, Maple, Juniper.

- Confirm 白杨 and 杨阳 play the new local Hen Local preview, not “这是一个充满
  希望的时代” and not “欢迎来到 Moxie Voice”.
- Confirm no extra voices appear.
- Confirm each English voice appears only for an English target and each Chinese
  voice appears for a Chinese target.

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

## Not ready to test yet

Account login, $49 checkout, billing portal, two-device management, and offline
license enforcement are implemented on the server side but not connected to a
deployed Supabase/Stripe environment. Apple Gatekeeper/notarization also awaits
Developer Program credentials. These are external setup blockers, not items to
pretend passed locally.
