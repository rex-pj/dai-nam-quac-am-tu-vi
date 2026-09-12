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
| `gloss-initial-restored.toml` | `parse` |
| `gloss-initial-lost.toml` | `parse` |
| `witness-*.toml` (7 file) | `witness` (chốt ⑥) |

**Giữ bằng tay** — người viết, chương trình chỉ đọc để lấy ngưỡng cho các chốt:

`label-typos.toml` · `image-glyphs.toml` · `jammed-text.toml` ·
`ambiguous-sub-entries.toml` · `gate4-index.toml` · `gate4-readings.toml` ·
`orthography-bridge.toml` · `errata-ban-in.toml` · `scan-verified.toml`

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

## `errata-ban-in.toml` — chính bản in tự sửa mình

Chỗ nầy có thẩm quyền hơn hết thảy những gì khác trong thư mục nầy, vì nó là lời của chính
tác giả chớ không phải lời một bản in sau hay một người chép sau. Bản in 1895-96 mang hai
bảng đính chính: **SAI SÓT** (cuốn 1, ảnh 14) và **ĐÍNH NGOA 訂 訛** (cuốn 2, ảnh 2-3), cộng
68 dòng. Chốt ⑦ đem dữ liệu ra dò với từng dòng ấy.

Vì sao phải có nó: chốt ⑥ không bắt được chỗ nầy. Bản 2026 với bản chép Wikisource **không
độc lập nhau** — đo ra thì trùng 4.662 trên 4.673 tự dạng, 716 trên 721 chữ hiếm chỉ hiện có
một lần, và 31 trên 32 mã Private-Use (những con số không mang nghĩa gì ngoài một bộ phông).
Hai bản chung một gốc, nên chỗ nào cả hai cùng sai thì chốt ⑥ mù. Cặp `故 Cô` "chị em bên
cha" với `姑 Cố` "sự cớ; cũ càng" là hai con chữ bị tráo, cả hai bản đều chép y như thợ sắp
chữ năm 1895 đã sắp. Chỉ có bảng SAI SÓT nói ra.

Chốt ⑦ bắt hai đằng. Dòng nào dữ liệu không theo mà không có `reason` thì đỏ; dòng nào dữ
liệu đã theo rồi mà còn `reason` thì cũng đỏ — hồ sơ cũ thôi làm chứng cho điều gì nữa.

## `gloss-initial-restored.toml` và `gloss-initial-lost.toml`

Hai hồ sơ nầy sanh ra từ một chỗ hỏng của lớp chữ bản 2026: 204 lời chú giải mất chữ hoa đầu
câu, vì lúc dựng lại lớp chữ, chữ ấy bị xếp thành một nhãn `dấu riêng` thứ nhì.

Bản in đặt `壓 Áp. c. Ngăn, giữ, đè, nhận xuống.` — một nhãn. Lớp chữ bản 2026 ghi
`壓 Áp c. n.` rồi xuống hàng `găn, giữ, đè, nhận xuống.`

`parse` trả chữ ấy về chỗ cũ khi nào hai điều cùng đúng: dòng mang hơn một nhãn, và chữ của
cái nhãn chót ghép với chữ kế nó thành một vần tiếng Việt thiệt (`N` + `găn` là vần *ng*, còn
`N` + `tụ` thì không có vần nào như vậy). Mỗi chỗ trả về đều biên vào
`gloss-initial-restored.toml` để còn dò lại được với ảnh trang in.

`gloss-initial-lost.toml` là phần còn lại: 143 lời chú giải vẫn mở đầu bằng chữ thường mà
chương trình không lấy lại được chữ hoa. Bản chép Wikisource cũng hỏng y như vậy, nên **chỉ
có mực trên giấy nói được**. Mở ra coi:

```
node tools/scan-page.mjs <cuốn> <ảnh> ra.png [x0 y0 x1 y1]
```

## `scan-verified.toml` — hồ sơ duy nhứt sửa chữ đưa ra cho đọc giả

Mọi hồ sơ khác ở đây đều chỉ ghi nhận và chờ. Cái nầy sửa, và nó có quyền sửa vì nó là thứ
duy nhứt nói **chính bản in đặt chữ gì**. Dòng nào chưa ký thì chưa sửa: `parse` chỉ áp dụng
dòng đã ký, dòng chưa ký thì mỗi lần chạy đều in ra kèm sẵn lịnh mở ảnh.

Chốt ⑧ giữ cho hồ sơ khỏi mục: mỗi dòng phải tìm thấy chuỗi `was` ở đúng trang `pdf_page`.
Không thấy thì đỏ — hoặc lớp chữ bản 2026 đã đổi, hoặc dòng ấy chép sai trang.

Dòng đầu tiên là `thế giới` ở trang 861. Bản in đặt `thế giái` cả hai chỗ (cuốn 2, ảnh 400,
cột phải). Chốt ⑥ không thấy gì, vì bản 2026 với bản chép Wikisource chung một gốc nên cùng
sai y nhau, mà trang ấy lại chưa hiệu đính. Cái gợi ra chỗ ngờ là **sách tự nghịch với
mình**: 13 chỗ khác viết `thế giái`, chữ đầu 界 đọc là *Giái*, và không có mục 界 *Giới* nào.

Chuyện nầy không làm thành chốt được. Sách trộn `sanh/sinh`, `chánh/chính`, `thiệt/thật`
khắp nơi — đo ra 68%, 75%, 45% dạng mới — nên dò kiểu ấy ra 51 chỗ ngờ mà gần hết là chánh
tả thiệt của tác giả. Một chốt hay báo động giả thì chẳng mấy lúc không ai thèm ngó. Nên chỗ
nầy để mắt người quyết, chỉ đưa cho người ta cái đèn (`tools/scan-page.mjs`) và cuốn sổ.
