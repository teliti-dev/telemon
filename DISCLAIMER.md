# Disclaimer

Telemon is free software licensed under the [GNU AGPL-3.0](LICENSE). This page repeats the license's warranty and
liability terms (sections 15 and 16) in plain language and adds the risks that are specific to Telemon. If this page and the
license ever disagree, the license text decides. This is not legal advice.

## 1. No warranty

Telemon is provided **"as is"**, without warranty of any kind, express or implied, including fitness for a particular purpose,
reliability, accuracy, security or availability. Pre-release versions (`v0.1.0-rc.x`) have not been run against a real
MikroTik router yet (see the README).

## 2. No liability

To the extent permitted by applicable law, Teliti, the authors and the contributors are **not liable** for any damage that
arises from using, or being unable to use, Telemon, including but not limited to:

- data that is lost, deleted, corrupted or exposed (including by a security breach or by unauthorized access);
- changes made to your routers, misconfiguration, service interruption or downtime;
- loss of revenue, customers or business, and any other direct, indirect or consequential damage;

even if they were told such damage was possible. By using Telemon you accept this and agree not to bring a claim against
them for such damage, again to the extent the law allows.

## 3. Risks you accept when you use Telemon

- **Writing to routers.** Telemon can add, edit and disable PPP secrets and profiles on your MikroTik routers. Read the
  review dialog before you apply a change. The SSH terminal extension runs commands with the **full rights of the router
  account**.
- **Network exposure.** LAN sharing and Public access make Telemon reachable by other people. You are responsible for your
  network, firewall, strong passwords and two-factor authentication.
- **Backups.** Keeping backups of Telemon's data folder and of your routers' configuration is your job. Telemon does not
  back up router configuration.
- **Personal data.** Customer names, IP and MAC addresses and PPPoE credentials may be personal data. You are responsible for
  handling them in line with the data-protection rules that apply to you.
- **AI assistant.** If you turn it on, the data you ask about is sent to the provider you choose, and its answers can be wrong.
  Check them before you act on them.
- **Downloads and updates.** The app downloads updates and, for Public access, the `cloudflared` program from their official
  sources, after checking signatures or checksums. You are responsible for deciding whether to use these features.

## 4. Extensions

Extensions are add-ons. **Each extension has its own publisher and its own license and terms** (free, open source, freemium,
paid or other); the license shown on an extension's page comes from its publisher. Telemon does not sell, verify or enforce
extension licenses. This disclaimer also applies to extensions, and Teliti is not responsible for extensions made by others.

## 5. What this page cannot do

Nothing here excludes or limits liability that cannot be excluded or limited by law (for example for fraud or intentional
harm), and the law where you live may give you rights that cannot be waived. If part of this page is invalid, the rest stays in
effect.

## 6. Other components

Telemon includes third-party components under their own licenses (for example `mikrotik-rs`, AGPL-3.0). See
[NOTICE](NOTICE) and the licenses of those components.
