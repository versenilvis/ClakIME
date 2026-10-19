#!/usr/bin/env bash
set -e

TMP_RESULT="/tmp/term_long_result.txt"
rm -f "$TMP_RESULT" /tmp/clak.log

RAW_TEXT="đây buoori sangs treen ddirnh nuis cao mang laij mootj carm giacs thaatj suwj thanh tharn vaf tuyeetj vowfi. nhuwxng tia nawngs vowis khoong gian roojng lowsn chieeus roji xuoongs khawps cais thung lungx, noowi cos nhuwxng tharm cor xanh mown mowrn vaf dofng suoosi nhor luowjn uoosn quanh qua tuwfng khe ddas. con nguwowfi noowi ddaay luoon chawm chir, kieen trif gawsn bos vowis ruoojng ddoofng, giuwx gifn truyeefn thoongs vawn hoas ddaamj ddaf barn sawsc cuar toor tieen. booj gox tieengs vieetj clak hoatj ddoojng voo cufng muowjt maf, pharn hoofi nhanh chongs vaf chinhs xacs treen cais mooi truwowfng terminal cuar heej ddieefu hanhf linux. duf laf vieest mootj ddoanj vawn hay laapj trinhf phaafn meefm phuwsc tapj, trari nghieemj gox phims luoon muowjt maf vaf ddem laij suwj thoari mais tuyeetj ddoosi cho nguwowif suwr dujng."

EXPECTED="đây buổi sáng trên đỉnh núi cao mang lại một cảm giác thật sự thanh thản và tuyệt vời. những tia nắng với không gian rộng lớn chiếu rọi xuống khắp cái thung lũng, nơi có những thảm cỏ xanh mơn mởn và dòng suối nhỏ lượn uốn quanh qua từng khe đá. con người nơi đây luôn chăm chỉ, kiên trì gắn bó với ruộng đồng, giữ gìn truyền thống văn hóa đậm đà bản sắc của tổ tiên. bộ gõ tiếng việt clak hoạt động vô cùng mượt mà, phản hồi nhanh chóng và chính xác trên cái môi trường terminal của hệ điều hành linux. dù là viết một đoạn văn hay lập trình phần mềm phức tạp, trải nghiệm gõ phím luôn mượt mà và đem lại sự thoải mái tuyệt đối cho người sử dụng."

DELAY=${1:-20}

echo "=== TEST GÕ ĐOẠN VĂN DÀI TRÊN TERMINAL (KITTY) ==="
echo "Độ dài: $(echo -n "$RAW_TEXT" | wc -c) ký tự raw, $(echo -n "$EXPECTED" | wc -w) từ"
echo "Tốc độ: ${DELAY}ms / phím"
echo ""

kitty --title "term_long_test" bash -c "sleep 0.2; cat > $TMP_RESULT" &
KPID=$!
sleep 1.2

fcitx5-remote -s clak || true

wtype -d "$DELAY" "$RAW_TEXT"
sleep 0.5
wtype -k Return
sleep 0.3
wtype -M ctrl -k d -m ctrl
sleep 0.5

kill $KPID 2>/dev/null || true
wait $KPID 2>/dev/null || true

ACTUAL=""
if [[ -f "$TMP_RESULT" ]]; then
    ACTUAL=$(cat "$TMP_RESULT" | tr -d '\r\n')
fi

echo "--- KẾT QUẢ THỰC TẾ ---"
echo "$ACTUAL"
echo ""

echo "--- SO SÁNH VỚI MONG ĐỢI ---"
if [[ "$ACTUAL" == "$EXPECTED" ]]; then
    echo "Trạng thái: 100% CHÍNH XÁC (MATCH HOÀN TOÀN)"
else
    echo "Trạng thái: CÓ KHÁC BIỆT"
    echo "Mong đợi: $EXPECTED"
    echo "Thực tế:  $ACTUAL"
fi
echo ""

echo "--- THỐNG KÊ ĐỘ TRỄ (LATENCY) TRONG LẦN TEST ---"
python3 scripts/tests/analyze_latency.py /tmp/clak.log
