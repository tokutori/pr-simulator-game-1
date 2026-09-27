# World asset生成

BPG-008/BPG-009で実装するhost-side toolである。
地理座標変換、切り出し、稜線を保持する簡略化、LOD生成、量子化、圧縮を担当する。
入力データの取得条件と標高基準を確認し、出典・処理条件・hashをmanifestに記録する。
生データをruntimeで取得・解析しない。
