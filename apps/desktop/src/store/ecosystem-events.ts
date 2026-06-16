import { queryKeys } from "../hooks/queries/queryKeys.ts";

type QueryInvalidator = {
  invalidateQueries: (options: { queryKey: readonly unknown[] }) => unknown;
};

export function invalidateEcosystemQueries(queryClient: QueryInvalidator) {
  void queryClient.invalidateQueries({ queryKey: queryKeys.capabilities });
  void queryClient.invalidateQueries({ queryKey: queryKeys.ecosystemItems });
  void queryClient.invalidateQueries({ queryKey: queryKeys.toolInventory });
}
