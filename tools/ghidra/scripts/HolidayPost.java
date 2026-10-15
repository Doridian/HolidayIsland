// Post-analysis for HOLIDAY.EXE: create functions for orphan code, i.e. code
// Ghidra disassembled (it was reached by flow from somewhere) but never turned
// into a function, such as targets that are only reached through VMT slots or
// event-handler pointers in the DFM-driven VCL.
//
// Unlike the Afterlife tooling there is no "function after RET" pass: Delphi 1
// puts string literals, VMTs and RTTI into the code segments next to the
// procedures, so undefined bytes after a RET are as likely data as code.
//@category HolidayIsland

import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.*;
import ghidra.program.model.mem.MemoryBlock;

public class HolidayPost extends GhidraScript {
	@Override
	protected void run() throws Exception {
		int created = 0;
		for (int pass = 0; pass < 5; pass++) {
			int n = createOrphanFunctions();
			created += n;
			if (n == 0) {
				break;
			}
		}
		println("created " + created + " functions for orphan code");
	}

	/** Create a function at every instruction that is outside any function and
	 *  is not reached by fall-through from the previous instruction. */
	private int createOrphanFunctions() throws Exception {
		Listing listing = currentProgram.getListing();
		FunctionManager fm = currentProgram.getFunctionManager();
		int created = 0;
		for (MemoryBlock b : currentProgram.getMemory().getBlocks()) {
			if (!b.isExecute()) {
				continue;
			}
			InstructionIterator it = listing.getInstructions(b.getStart(), true);
			while (it.hasNext() && !monitor.isCancelled()) {
				Instruction insn = it.next();
				Address a = insn.getAddress();
				if (!b.contains(a)) {
					break;
				}
				if (fm.getFunctionContaining(a) != null) {
					continue;
				}
				Instruction prev = listing.getInstructionBefore(a);
				if (prev != null && a.equals(prev.getFallThrough())) {
					continue;
				}
				Function f = createFunction(a, null);
				if (f != null) {
					created++;
					it = listing.getInstructions(f.getBody().getMaxAddress().next(), true);
				}
			}
		}
		return created;
	}
}
