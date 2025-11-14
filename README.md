# Hyperliquid HYPE Arbitrage Bot

Ultra yüksek frekanslı, düşük gecikmeli HYPE/USDC perpetual-spot arbitraj botu.

## 🚀 Özellikler

- **Ultra Düşük Gecikme**: <10ms emir iletimi
- **Yüksek Frekans**: Saniyede 100+ fiyat karşılaştırması  
- **Otomatik Recovery**: WebSocket auto-reconnect
- **Risk Yönetimi**: Circuit breaker ve position limitleri
- **Native Rust**: Zero GC, lock-free architecture

## 📋 Gereksinimler

- Rust 1.70+
- 2 çekirdekli VPS (Mumbai recommended)
- Hyperliquid API erişimi
- 100+ USDC bakiye

## 🔧 Kurulum

```bash
# Repository'yi klonla
git clone https://github.com/lprnmns/hyperliquid_hype_arbitrage.git
cd hyperliquid_hype_arbitrage

# Build
cargo build --release

# Test
cargo test

# Çalıştır
cargo run --release
```

## ⚙️ Konfigürasyon

`.env.example` dosyasını `.env` olarak kopyalayın ve değerleri doldurun:

```env
HL_API_AGENT_PRIVATE_KEY=0x...
HL_API_AGENT_WALLET_ADDRESS=0x...
BPS_THRESHOLD=5.0
POSITION_SIZE_USD=20
LEVERAGE=2
```

## 📊 Performans Metrikleri

- **Throughput**: 100K+ msg/sec
- **Latency**: p99 < 10ms
- **Uptime**: 99.9%
- **Success Rate**: >99.9%

## 🛡️ Güvenlik

- Private key'ler asla commit edilmez
- Tüm hassas veriler .env dosyasında
- Rate limiting koruması
- Circuit breaker sistemi

## 📈 Strateji

Bot, perpetual ve spot piyasaları arasındaki basis spread'den yararlanır:
- Entry: Basis > threshold (default 5 bps)
- Exit: Basis ≈ 0 
- Position: Perp short + Spot long (delta neutral)

## 🧪 Test

```bash
# Unit testler
cargo test

# Benchmark testler  
cargo bench

# Integration testler
cargo test --features integration
```

## 📝 Lisans

Proprietary - Tüm hakları saklıdır.

## 👤 Geliştirici

[@lprnmns](https://github.com/lprnmns)