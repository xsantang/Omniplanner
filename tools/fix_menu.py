"""Fix the menu block in main.rs - add ajustar_tasa option and fix corrupted emoji."""
import re

path = r"C:\Users\elxav\proyectos\omniplanner\src\main.rs"
with open(path, "r", encoding="utf-8") as f:
    content = f.read()

changes = 0

# 1. Fix corrupted emoji on the "Importar" line
# The \ufffd (replacement char) needs to become 📂
content_new = re.sub(
    r'"\ufffd\s+Importar desde CSV \(Excel convertido\)"',
    '"📂  Importar desde CSV (Excel convertido)"',
    content,
)
if content_new != content:
    content = content_new
    changes += 1
    print("1. Fixed corrupted emoji")

# 2. Add "Ajustar tasa" option after "Editar pago"
old_menu = '''"✏️   Editar pago de un mes",
            "💵  Configurar ingreso quincenal",'''
new_menu = '''"✏️   Editar pago de un mes",
            "⚙️   Ajustar tasa de interés",
            "💵  Configurar ingreso quincenal",'''
if old_menu in content:
    content = content.replace(old_menu, new_menu, 1)
    changes += 1
    print("2. Added 'Ajustar tasa' option")

# 3. Fix the match block numbering
old_match = """Some(4) => rastreador_editar_pago(state),
            Some(5) => rastreador_ingreso(state),
            Some(6) => rastreador_exportar(state),
            Some(7) => rastreador_importar_csv(state),
            Some(8) => rastreador_eliminar(state),"""
new_match = """Some(4) => rastreador_editar_pago(state),
            Some(5) => rastreador_ajustar_tasa(state),
            Some(6) => rastreador_ingreso(state),
            Some(7) => rastreador_exportar(state),
            Some(8) => rastreador_importar_csv(state),
            Some(9) => rastreador_eliminar(state),"""
if old_match in content:
    content = content.replace(old_match, new_match, 1)
    changes += 1
    print("3. Fixed match block")

with open(path, "w", encoding="utf-8") as f:
    f.write(content)

print(f"\nDone: {changes} changes applied")
