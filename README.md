# 🐀 Houserat

A daemon that monitors network traffic for DHCP and ARP packets from known devices and sends a notification
using Telegram when these devices connect or disconnect.

## 🚀 Usage

1. Install using a package manager:
   * **Arch Linux**: [AUR](https://aur.archlinux.org/packages/houserat/), e.g. `yay -S houserat`
   * **Cargo**: `cargo install houserat` (note that you'll have to manually install the service and
     config files)
1. Edit configuration at `/etc/houserat/config.toml` ([example config](config.example.toml)).
1. Enable and start service: `systemctl enable --now houserat`.

## 💫 How It Works

Houserat detects devices connecting to the network when they send a DHCP request packet. It will
then notify that device's subscriber and start polling this device to detect when it goes
away. Since phones don't always respond to PING packets houserat uses ARP requests which all devices
must respond to.

When several ARP requests go unanswered the device is considered disconnected and a notification is
sent to the subscriber.

## 💤 Anti-Spam

Houserat has several features designed to reduce notification spam:
* Configurable *cooldown* during which no new notifications are sent, for example if a device
  reconnects soon after its initial connection only 1 notification is sent.
* Configurable *quiet period* during which messages are sent without sound notifications. This can be
  used to avoid having noisy Telegram notifications at night.
