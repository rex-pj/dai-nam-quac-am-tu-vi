# Hồ sơ chờ người đối chứng

Chỗ nào lớp văn bản gốc không nói rõ, chương trình **dừng lại và biên vào đây**, chớ không
điền một giá trị nghe cho xuôi tai. Mỗi dòng trong thư mục nầy là một chỗ như vậy.

Ô `verified_by` là chữ ký. Để trống nghĩa là **chưa ai ngó tới**. Ký vào nghĩa là: đã mở ảnh
trang in ra coi, và nhận trách nhiệm về điều mình ghi.

## Hai giống hồ sơ, đừng lẫn

**Sinh tự động** — đừng sửa tay phần thân, lần chạy sau sẽ ghi đè:

| File | Do ai sinh |
|---|---|
| `residual-placeholders.toml` | `parse` |
| `witness-*.toml` (6 file) | `witness` (chốt ⑥) |

**Giữ bằng tay** — người viết, chương trình chỉ đọc để lấy ngưỡng cho các chốt:

`label-typos.toml` · `image-glyphs.toml` · `jammed-text.toml` ·
`ambiguous-sub-entries.toml` · `gate4-index.toml` · `gate4-readings.toml` ·
`orthography-bridge.toml`

## Chữ ký không mất khi sinh lại

Mỗi dòng trong hồ sơ sinh tự động mang một ô `key` tả **chính chỗ sai ấy** — số trang cộng
với đoạn chữ — chớ không phải số thứ tự. Sinh lại thì chữ ký theo `key` mà về đúng dòng của
nó. Nhờ vậy thứ tự có đảo, số dòng có thêm bớt, chữ ký vẫn còn.

Chỗ sai nào đã hết thì chữ ký của nó cũng đi theo luôn. Cố ý như vậy: nó bảo chứng cho một
điều không còn nữa, giữ lại thì có ngày nó dán nhầm lên chỗ chưa ai coi.

## `witness-*.toml` — chốt ⑥ nói gì

Nhân chứng là bản chép Wikisource của **bản in gốc 1895–96**, chụp lại trong
`data/wikisource/` (số hiệu bản sửa từng trang ghi ở `PROVENANCE.json`). Bản chép ấy mang
giấy phép **CC BY-SA**: dùng để đối chiếu thì thong thả, bưng chữ của họ vào dữ liệu thì
vướng nghĩa vụ ghi công và chia sẻ tương tự.

Hai bản in là hai bản khác nhau — bản 1895 có phần *Bổ di* thêm mục mà bản sau gộp vào — nên
lệch nhau là lẽ thường. Chỉ hai loại dưới đây mới là **lời tố rằng mình đọc sai trang**:

| File | Nghĩa |
|---|---|
| `witness-column-boundary.toml` | bản 1895 ngắt cột Hán / Quốc ngữ ở chỗ khác |
| `witness-glyph-differs.toml` | hai bên cùng có mã chữ mà mã khác nhau |

Bốn file còn lại là **tư liệu**, không phải lỗi: `entry-only-there`, `entry-only-here`,
`glyph-unencodable-there`, và `shape-note-available`.

## `witness-shape-note-available.toml` — chỗ duy nhứt hồ sơ chảy ngược vào dữ liệu

29 mục từ mà bản 2026 phải đặt ảnh vì chữ chưa có mã Unicode. Người chép bản 1895 cũng bó tay
ở 28 trong số đó, và thay vì đoán bừa một mã, họ **tả hình chữ** — `⿰口胖`, `⿱壯卵`.

Ký vào một dòng ở đây là cho phép lời tả ấy vào cơ sở dữ liệu và hiện lên trang mục từ.
`import` **chỉ lấy dòng đã ký**; dòng chưa ký nằm yên đây, không ai đọc thấy. Đó vừa là phép
giữ cho dự án không nói thay người khác, vừa là chỗ giải quyết chuyện giấy phép: không có chữ
ký thì không có gì được phát hành.

Lời tả là **tả hình, không phải mã chữ**. Ký vào không có nghĩa là gán mã Unicode cho tự dạng
ấy — chương trình vẫn để trống mã, y như cũ.
