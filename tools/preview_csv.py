"""Preview of what the import will look like."""
import csv

with open("datos_deudas.csv", encoding="utf-8-sig") as f:
    reader = csv.DictReader(f)
    cuentas = {}
    for row in reader:
        c = row["cuenta"]
        if c not in cuentas:
            cuentas[c] = {"meses": 0, "first_saldo": float(row["saldo"]), "last_saldo": 0}
        cuentas[c]["meses"] += 1
        cuentas[c]["last_saldo"] = float(row["saldo"])

print(f"{len(cuentas)} cuentas encontradas:")
total_i = total_f = 0
for c in sorted(cuentas.keys()):
    d = cuentas[c]
    si, sf = d["first_saldo"], d["last_saldo"]
    total_i += si
    total_f += sf
    if sf > si + 100:
        arrow = "CRECIO"
    elif sf < si * 0.5:
        arrow = "BAJO"
    else:
        arrow = "ESTABLE"
    print(f"  {c:25s} ${si:>10,.2f} -> ${sf:>10,.2f}  ({d['meses']:2d} meses) {arrow}")
print(f"  {'='*75}")
print(f"  {'TOTAL':25s} ${total_i:>10,.2f} -> ${total_f:>10,.2f}")
