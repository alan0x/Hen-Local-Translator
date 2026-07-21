# Account, Device, and License Recovery

This is the launch support runbook for a two-device, seven-day offline license.

## Customer self-service

1. Sign in through the secure browser flow.
2. Open Account in Settings to see active devices and last-used times.
3. Deactivate a computer that is no longer used, then activate the replacement.
4. Use the billing portal to update payment details or cancel.

## Common recovery cases

- **Reinstall on the same Mac:** recover the device key from macOS Keychain and
  refresh the existing activation. Do not consume a new slot.
- **Keychain deleted or Mac replaced:** sign in, deactivate the old device, and
  activate the new key. If both slots are inaccessible, support may verify the
  account and deactivate an old record.
- **Offline:** the most recent signed lease works until its displayed expiry,
  for no more than seven days. Changing the computer clock must not extend it.
- **Corrupt or missing license:** preserve local transcripts and settings, ask
  the customer to reconnect and refresh, and never instruct them to delete all
  app data as the first step.
- **Third device blocked:** show both active device names and offer deactivation;
  do not silently replace a device.

## Support safeguards

Support may review account ID, device ID/name, activation times, license status,
and billing state. Never request audio, transcript content, passwords, complete
payment-card data, private device keys, or signing keys. Every staff-initiated
device reset must create an audit event.
