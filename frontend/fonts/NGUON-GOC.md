# Font của bản in — xuất xứ

Bốn file `.ttf` trong thư mục này **không tải từ đâu về**. Chúng được trích ra từ chính
`docs/Đại Nam Quốc Âm Tự Vị (2026).pdf`, bằng `pipeline/font`:

```
cargo run -p dnqatv-font --release -- "docs/<PDF>" data/entries.jsonl frontend/fonts
```

## Vì sao phải mang theo font

27 tự dạng của cuốn tự vị mang mã **vùng dùng riêng** (PUA, U+F0000+). Mã PUA không có
nghĩa chuẩn — chúng chỉ trỏ đúng glyph *bên trong* font của bản in. Máy không có font thì
hiện ô vuông ▯; tệ hơn, máy có font khác cũng định nghĩa mã đó thì hiện **một chữ sai** mà
không báo gì. Với từ điển, đó là hỏng dữ liệu ở tầng nhìn.

## Vì sao hai font chứ không phải một

Nôm Na Tống gánh gần hết cuốn sách, nhưng **không có đủ mọi chữ**: đo trên dữ liệu thật,
26 điểm mã (15 BMP + 11 Ext-B) vắng trong nó, và bản in 2026 đặt chúng bằng **BabelStone
Han**. Cả hai font đều nằm trong cùng một PDF, nên cả hai đều được trích và cắt ở đây.

Không có file thứ tư ấy thì 26 chữ đó rơi xuống font hệ thống — mỗi máy một kiểu, và nhiều
máy không có font nào phủ được, nên chúng hiện ra ô vuông.

## Số liệu

| file | nguồn | chữ | kích thước | outline đã đối chiếu với bản gốc |
|---|---|---|---|---|
| `NomNaTong-bmp.ttf`       | Nôm Na Tống    | 4.112 | 1.726 KB | 4.097 |
| `NomNaTong-extb.ttf`      | Nôm Na Tống    |   855 |   408 KB |   844 |
| `NomNaTong-pua.ttf`       | Nôm Na Tống    |    27 |    18 KB |    27 |
| `BabelStoneHan-gap.ttf`   | BabelStone Han |    26 |    37 KB |    26 |

Font gốc trong PDF: Nôm Na Tống **15.917.208 byte / 32.951 glyph**, BabelStone Han
**51.971.284 byte / 64.789 điểm mã**, cả hai đều không bị subset.

Toàn bộ 4.994 điểm mã Hán-Nôm mà `data/entries.jsonl` dùng đều được phủ.

## Chốt kiểm

`tests/unit/font/coverage.rs` khẳng định hai chiều:

- mọi chữ trong dữ liệu đều có font phủ — nạp lại dữ liệu mà xuất hiện chữ mới thì test đỏ,
  kèm danh sách chữ và trang in để biết chạy lại `dnqatv-font`;
- không file nào mang chữ dữ liệu không dùng — nếu có thì font đã được cắt cho bộ dữ liệu khác.

Test tự bỏ qua khi thiếu `data/` hoặc thiếu font, nên máy chưa chạy `parse` vẫn `cargo test`
được. `data/` không nằm trong repo nên trên CI nó bỏ qua; chốt này chạy ở máy có dữ liệu.

## Khai `@font-face`

Không khai sẵn trong `main.css`. `FontSet::scan` soi thư mục lúc khởi động và sinh
`@font-face` cho đúng những file có thật. Ba file Nôm khai theo dải cố định; file gap khai
**danh sách 26 điểm mã đọc thẳng từ `cmap` của chính nó**, nên không có bản sao thứ hai của
danh sách ấy để lệch.

File gap khai dưới đúng tên họ của nó, `BabelStone Han`, chứ không mượn tên `Nom Na Tong` —
glyph là của ai thì khai của người ấy. `--nom` trong `main.css` xếp `"BabelStone Han"` ngay
sau `"Nom Na Tong"` để file ta ship luôn thắng font cùng tên có sẵn trên máy người đọc: cùng
một chữ phải hiện giống nhau ở mọi máy.

## Bản quyền và giấy phép

Cả hai giấy phép dưới đây **đọc thẳng từ bảng `name` của chính font**, không lấy từ trí nhớ.
Bảng `name` được giữ nguyên qua phép cắt, nên bản quyền, giấy phép và tên họ font đi theo file.

| font | bản quyền | giấy phép |
|---|---|---|
| Nôm Na Tống | © 2011–2026 The Vietnamese Nom Preservation Foundation — Nom Na Group | **MIT** |
| BabelStone Han | © 1994–1999 Arphic Technology Co., Ltd.; © 2009–2025 Andrew West | **Arphic Public License** |

Toàn văn hai giấy phép nằm cạnh font: `LICENSE-NomNaTong-MIT.txt` và `ARPHICPL.txt`, cũng
trích ra từ bảng `name` của chính file.

### Nghĩa vụ khi cắt font

Một bản cắt là một **bản sửa đổi**, và Arphic Public License nói rõ ở §2(a):

> You must insert a prominent notice in each modified file stating how and when you changed
> that file.

Ghi trong repo không đủ — file mới là thứ đến tay người đọc. Nên `pipeline/font` viết ghi chú
ấy vào **nameID 10 (Description)** của cả bốn file: cắt từ font nào, sửa ngày nào, bỏ những gì,
và khẳng định không outline nào bị đổi. Đọc ra bằng bất kỳ trình xem font nào.

Arphic §1 còn đòi giữ nguyên file giấy phép trong mọi bản sao — đó là lý do `ARPHICPL.txt`
nằm cạnh font chứ không chỉ nằm trong bảng `name`.

Nôm Na Tống theo MIT thì nhẹ hơn: chỉ cần giữ thông báo bản quyền và giấy phép. Nhưng ghi chú
sửa đổi vẫn được viết vào, vì nó đúng và vì người sau cần biết file này đã bị cắt.

`tests/unit/font/coverage.rs` khẳng định cả ba thứ ấy còn nguyên trong từng file — bảng `name`
bị bỏ đi thì tiết kiệm được vài KB và làm hỏng giấy phép, mà không gì khác nhận ra.

Nôm Na Tống còn mang thông báo nhãn hiệu (nameID 7): *"Nom Na Tong is a trademark of The
Vietnamese Nom Preservation Foundation."* Bản cắt giữ nguyên tên họ font, đúng như bản gốc.

## Dựng lại

Xoá thư mục này thì trang vẫn chạy: `FontSet::scan` không thấy file thì không khai
`@font-face`, và trang mục từ tự hiện lại lời nhắc "cần font Nôm Na Tống".
